//! Subscribe to a Web3 RPC and do something when an event is seen
//!
//! For now, this is hard coded for LlamaNodes Payment Factory deposit events,
//! but it could be useful generally.
//!
//! # Running
//!
//! ```bash
//! export W3TTT_PROXY_URLS=eth.llamarpc.com,polygon.llamarpc.com
//! cargo run
//! ```
//!
//! # Development
//!
//! The README.md is kept up-to-date with `cargo readme > README.md`
//!
//! # Questions (and hopefully Answers)
//!
//! - should this care about confirmation depth?
//! - should this care about orphaned transactions?
//!
//! # Todo
//!
//! - [x] proper logging
//! - [ ] petgraph for tracking forks?
//! - [x] retry rather than exit
//! - [ ] handle orphaned transactions
//!
use anyhow::Context;
use dotenv::dotenv;
use ethers::{
    abi::ethereum_types::BloomInput,
    prelude::{abigen, LogMeta},
    providers::{Middleware, Provider, StreamExt, Ws},
    types::{Address, Block, TxHash, ValueOrArray},
};
use futures::future::try_join_all;
use reqwest::Client;
use sentry::types::Dsn;
use serde::Deserialize;
use std::{env, str::FromStr, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{debug, error, info, info_span, trace, Instrument, Level};
use tracing_subscriber::{prelude::*, EnvFilter, FmtSubscriber};

type EthersProviderWs = Provider<Ws>;

// TODO: do this at runtime so we can listen to multiple events
abigen!(
    LlamaNodes_PaymentContracts_Factory,
    r#"[
        event PaymentReceived(address account, address token, uint256 amount)
    ]"#
);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // use dotenv to get config
    let _ = dotenv().ok();

    // optional, but if sentry dsn is set, it MUST parse
    let sentry_dsn = env::var("W3TTT_SENTRY_DSN")
        .ok()
        .map(|x| x.parse::<Dsn>().unwrap());

    // set up sentry connection
    let _sentry_guard = sentry::init(sentry::ClientOptions {
        dsn: sentry_dsn,
        release: sentry::release_name!(),
        // Enable capturing of traces
        // we set to 100% here while we develop, but production will likely want to configure this smaller
        traces_sample_rate: 1.0,
        ..Default::default()
    });

    // create a subscriber that uses the RUST_LOG env var for setting levels
    // print a compact output to the terminal
    // Register the Sentry tracing layer to capture breadcrumbs, events, and spans
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish()
        .with(tracing_subscriber::fmt::layer().compact())
        .with(sentry_tracing::layer())
        .init();

    let proxy_urls = env::var("W3TTT_PROXY_URLS")
        .context("Setting W3TTT_PROXY_URLS in your environment is required")?;

    let http_client = reqwest::Client::new();

    let handles = proxy_urls.split(',').map(|proxy_url| {
        let f = run_forever(http_client.clone(), proxy_url.to_owned());

        // TODO: prometheus metrics?
        // TODO: dotfile exporter for the chain?

        tokio::spawn(f)
    });

    try_join_all(handles).await?;

    Ok(())
}

#[derive(Deserialize)]
struct StatusPage {
    payment_factory_address: Option<Address>,
}

async fn run_forever(http_client: Client, proxy_url: String) {
    loop {
        if let Err(err) = run(&http_client, &proxy_url).await {
            error!("{} errored! {:?}", proxy_url, err);
        }
        sleep(Duration::from_secs(10)).await;
    }
}

async fn run(http_client: &Client, proxy_url: &str) -> anyhow::Result<()> {
    let (http_scheme, ws_scheme) = if proxy_url.contains("localhost") {
        ("http", "ws")
    } else {
        ("https", "wss")
    };

    let http_url = format!("{}://{}", http_scheme, proxy_url);
    let ws_url = format!("{}://{}", ws_scheme, proxy_url);

    let status_url = format!("{}/status", http_url);

    let status_json: StatusPage = http_client
        .get(status_url)
        .send()
        .await?
        .json()
        .await
        .context("unexpected format of /status")?;

    let factory_address = status_json.payment_factory_address.unwrap_or_else(|| {
        "0x4e3BC2054788De923A04936C6ADdB99A05B0Ea36"
            .parse()
            .unwrap()
    });

    // TODO: acquire mysql lock? multiple reports for the same transaction doesn't really hurt anything

    // create a websocket connection to an rpc provider
    let provider = Arc::new(
        Provider::<Ws>::connect_with_reconnects(ws_url, usize::MAX)
            .await
            .context("failed connecting to websocket")?,
    );

    // TODO: query mysql (or maybe redis) to know the last row we processed
    // 40157242 (march 9, 2023) is when 0x4e3bc2054788de923a04936c6addb99a05b0ea36 was created
    let mut last_processed = provider
        .get_block(41380731)
        .await
        .context("failed fetching last processed block")?
        .context("last_processed block should always exist")?;

    let factory = LlamaNodes_PaymentContracts_Factory::new(factory_address, provider.clone());

    let mut new_heads_sub = provider.subscribe_blocks().await?;

    while let Some(new_head) = new_heads_sub.next().await {
        // TODO: will need to handle reorgs. will need to find the common ancestor
        // TODO: lag by X blocks
        let new_head_number = new_head.number.unwrap().as_u64();
        let new_head_hash = new_head.hash.unwrap();

        let last_number = last_processed.number.unwrap().as_u64();
        let last_hash = last_processed.hash.unwrap();

        if new_head_hash == last_hash {
            // we've already seen this block
            continue;
        }

        // TODO: don't exit on error. sleep and retry
        for i in last_number..new_head_number {
            let block = provider
                .get_block(i)
                .await
                .context("failed fetching old block")?
                .context("there should always be a block")?;

            let span = info_span!(
                "process_block",
                num=%block.number.unwrap(),
                hash=?block.hash.unwrap(),
                http_url,
            );

            process_block(&block, http_client, &factory, &http_url)
                .instrument(span)
                .await?;

            // TODO: save last_processed somewhere external in case this process exits
            last_processed = block;
        }

        let span = info_span!(
            "process_block",
            num=%new_head.number.unwrap(),
            hash=?new_head.hash.unwrap(),
            http_url,
        );

        process_block(&new_head, http_client, &factory, &http_url)
            .instrument(span)
            .await?;

        last_processed = new_head;

        // TODO: save last_processed somewhere external in case this process exits
    }

    Ok(())
}

pub async fn process_block(
    block: &Block<TxHash>,
    http_client: &Client,
    factory: &LlamaNodes_PaymentContracts_Factory<EthersProviderWs>,
    proxy_http_url: &str,
) -> anyhow::Result<()> {
    debug!("checking logs_bloom");

    // TODO: can we check multiple inputs at the same time?
    let logs_bloom = block
        .logs_bloom
        .context("blocks here should always have a bloom")?;

    let payment_received_filter = factory.payment_received_filter();

    let payment_received_topic0 = if let ValueOrArray::Value(Some(x)) =
        payment_received_filter.filter.topics[0].as_ref().unwrap()
    {
        x
    } else {
        panic!("topic0 should always be set");
    };

    // check the topic bloom first because it has more bits and so hopefully has less common false positives
    let topics_bloom_input = BloomInput::Hash(payment_received_topic0.as_fixed_bytes());

    if !logs_bloom.contains_input(topics_bloom_input) {
        trace!("block does not contain logs using the payment_received event");
        return Ok(());
    }

    // next check the contract address
    let factory_address = factory.address();
    let address_bloom_input = BloomInput::Raw(factory_address.as_bytes());

    if !logs_bloom.contains_input(address_bloom_input) {
        trace!("block does not contain logs using the factory address");
        return Ok(());
    }

    // bloom filters will never have a false negative
    // there can be false positives, but bloom filter checks cut out a lot of unnecessary eth_getLogs
    trace!("bloom filters passed");

    let block_hash = block.hash.unwrap();

    // filter for the logs only on this new head block
    let payment_received_filter = factory.payment_received_filter().at_block_hash(block_hash);

    // rust-analyzer loses this type
    let logs_with_meta: Vec<(PaymentReceivedFilter, LogMeta)> =
        payment_received_filter.query_with_meta().await?;

    trace!(?logs_with_meta, "from payment received filter");

    for (_, log_meta) in logs_with_meta {
        let txid = log_meta.transaction_hash;

        info!(?txid, "submitting transaction");

        // TODO: submit the txid to a new endpoint on web3-proxy
        // TODO: post in the body instead?
        let r = http_client
            .post(format!("{}/user/balance/{:?}", proxy_http_url, txid))
            .send()
            .await?;

        info!(?r, "transaction submitted");

        let j = r.text().await?;

        info!(?j, "transaction submitted");
    }

    Ok(())
}
