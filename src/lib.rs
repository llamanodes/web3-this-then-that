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
use deadpool_redis::{redis::AsyncCommands, Config, Pool, Runtime};
use derivative::Derivative;
use ethers::{
    abi::{ethereum_types::BloomInput, AbiEncode},
    prelude::{abigen, LogMeta},
    providers::{Middleware, Provider, StreamExt, Ws},
    types::{Address, Block, TxHash, H256, U64},
};
use reqwest::Client;
use serde::Deserialize;
use std::{cmp::Ordering, env, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::instrument;
use tracing::{debug, error, info, info_span, trace, warn, Instrument};

pub type EthersProviderHttp = ethers::providers::Provider<ethers::providers::Http>;
pub type EthersProviderWs = ethers::providers::Provider<ethers::providers::Ws>;

// TODO: refactor to listen to arbitrary events
abigen!(
    LlamaNodes_PaymentContracts_Factory,
    r#"[
        event PaymentReceived(address indexed account, address token, uint256 amount)
    ]"#
);

pub fn get_redis_pool<T: Into<String>>(url: T) -> Arc<Pool> {
    let cfg = Config::from_url(url);

    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();

    Arc::new(pool)
}

#[derive(Deserialize)]
struct StatusJson {
    chain_id: u64,
    head_block_num: Option<U64>,
    payment_factory_address: Option<Address>,
}

pub async fn run_forever(http_client: Client, proxy_url: String, redis_pool: Option<Arc<Pool>>) {
    info!("starting {}", proxy_url);
    loop {
        if let Err(err) = run(&http_client, &proxy_url, redis_pool.as_deref()).await {
            // TODO: the useful spans are all gone here
            error!(?err, "{} errored! {}", proxy_url, err);
        }
        sleep(Duration::from_secs(60)).await;
    }
}

// TODO: fix this syntax
#[instrument(skip(http_client, redis_pool))]
async fn run(
    http_client: &Client,
    proxy_url: &str,
    redis_pool: Option<&Pool>,
) -> anyhow::Result<()> {
    if proxy_url.starts_with("http") || proxy_url.starts_with("ws") {
        panic!("invalid proxy_url. do not include the scheme");
    }

    let (http_scheme, ws_scheme) = if proxy_url.contains("localhost") {
        ("http", "ws")
    } else {
        ("https", "wss")
    };

    let proxy_url_maybe_with_key = match env::var("W3TTT_RPC_KEY") {
        Ok(rpc_key) => format!("{}/rpc/{}", proxy_url, rpc_key),
        Err(_) => proxy_url.to_string(),
    };

    let http_url = format!("{}://{}", http_scheme, proxy_url_maybe_with_key);
    let ws_url = format!("{}://{}", ws_scheme, proxy_url_maybe_with_key);

    let status_url = format!("{}://{}/status", http_scheme, proxy_url);
    let mut status_json: StatusJson = http_client
        .get(&status_url)
        .send()
        .await
        .context("unable to get /status")?
        .json()
        .await
        .context("unexpected format of /status")?;

    let chain_id = status_json.chain_id;

    let mut factory_address = status_json.payment_factory_address;

    while factory_address.is_none() {
        warn!(
            "no factory address for chain {}. Trying again in 60 seconds",
            chain_id
        );

        sleep(Duration::from_secs(60)).await;

        status_json = http_client
            .get(&status_url)
            .send()
            .await
            .context("unable to get /status")?
            .json()
            .await
            .context("unexpected format of /status")?;

        factory_address = status_json.payment_factory_address;
    }

    let factory_address = factory_address.unwrap();

    // TODO: acquire mysql lock? multiple reports for the same transaction doesn't really hurt anything

    // create a websocket connection to an rpc provider
    let provider = Arc::new(
        Provider::<Ws>::connect_with_reconnects(ws_url, usize::MAX)
            .await
            .context("failed connecting to websocket")?,
    );

    let head_block_num = status_json
        .head_block_num
        .ok_or(anyhow::anyhow!("no head block in status"))?;

    let mut last_processed = LastProcessed::try_new(
        chain_id,
        &factory_address,
        &head_block_num,
        &provider,
        redis_pool,
    )
    .await
    .context("failed creating last_processed")?;

    info!(?last_processed);

    let factory = LlamaNodes_PaymentContracts_Factory::new(factory_address, provider.clone());

    let mut new_heads_sub = provider.subscribe_blocks().await?;

    while let Some(new_head) = new_heads_sub.next().await {
        // the block header does not contain everything in the block. we need to call get_block for that

        let new_block;

        let mut error_count = 0;

        loop {
            let new_hash = new_head.hash.unwrap();

            match provider.get_block(new_hash).await {
                Ok(Some(x)) => {
                    new_block = x;
                    break;
                }
                err => {
                    // TODO: wtf. how is this happening the first time? and if this is cached, that is very bad!
                    error_count += 1;
                    error!(?err, %error_count, ?new_hash, "no block!");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }

        debug!(new_hash=?new_block.hash, new_num=?new_block.number, %error_count);

        // TODO: will need to handle reorgs. will need to find the common ancestor
        // TODO: lag by X blocks
        let new_head_number = new_block.number.unwrap().as_u64();
        let new_head_hash = new_block.hash.unwrap();

        // TODO: don't unwrap
        let last_number = last_processed.number().unwrap();
        let last_hash = *last_processed.hash().unwrap();

        if new_head_hash == last_hash {
            // we've already seen this block
            continue;
        }

        for i in last_number..new_head_number {
            let old_block = provider
                .get_block(i)
                .await
                .context("failed fetching old block")?
                .context("there should always be a block")?;

            let span = info_span!(
                "process_block",
                num=%old_block.number.unwrap(),
                hash=?old_block.hash.unwrap(),
            );

            process_block(&old_block, http_client, &factory, &http_url)
                .instrument(span)
                .await?;

            last_processed.set(old_block).await?;
        }

        let span = info_span!(
            "process_block",
            num=%new_block.number.unwrap(),
            hash=?new_block.hash.unwrap(),
        );

        // TODO: if we log the error here, then the span will be included
        process_block(&new_block, http_client, &factory, &http_url)
            .instrument(span)
            .await?;

        last_processed.set(new_block).await?;
    }

    Ok(())
}

#[derive(Derivative)]
#[derivative(Debug)]
struct LastProcessed<'a> {
    block: Block<TxHash>,
    num_key: String,
    hash_key: String,
    #[derivative(Debug = "ignore")]
    redis_pool: Option<&'a Pool>,
}

impl<'a> LastProcessed<'a> {
    async fn try_new(
        chain_id: u64,
        factory_address: &Address,
        head_block_num: &U64,
        provider: &EthersProviderWs,
        redis_pool: Option<&'a Pool>,
    ) -> anyhow::Result<LastProcessed<'a>> {
        let num_key = format!(
            "W3TTT:2:{}:{:?}:LastProcessedNum",
            chain_id, factory_address
        );
        let hash_key = format!(
            "W3TTT:2:{}:{:?}:LastProcessedHash",
            chain_id, factory_address
        );

        let mut last_block_hash_or_number = None;

        // TODO: make it easy to force a block number while debugging
        // if chain_id == 1 {
        //     last_block_hash_or_number = None;
        // } else if chain_id == 137 {
        //     last_block_hash_or_number = Some(45741167);
        // }

        // get the block hash from the redis
        // TODO: something more durable than redis could work, but re-running this isn't that big of a problem
        if let Some(redis_pool) = redis_pool {
            let mut conn = redis_pool.get().await?;

            // TODO: do something with hash_key? do something with an error
            let x: Option<String> = conn.get(&num_key).await?;

            // TODO: better to U64 encoded as hex, or encode as decimal? decimal seems easier since we aren't using json
            let x: Option<Result<u64, _>> = x.map(|x| x.parse());

            let x: Option<u64> = match x {
                Some(Ok(x)) => Some(x),
                Some(Err(err)) => {
                    warn!(?err, "unable to parse saved value");
                    None
                }
                None => None,
            };

            last_block_hash_or_number = x;
        }

        // if no data in redis, find when the factory was deployed
        if last_block_hash_or_number.is_none() {
            info!("no last block found. using the deploy block");

            let deploy_block_num =
                binary_search_eth_get_code(provider, factory_address, head_block_num)
                    .await?
                    .as_u64();

            last_block_hash_or_number = Some(deploy_block_num)
        }

        let last_block_hash_or_number = last_block_hash_or_number.context("no start block")?;

        let block = provider
            .get_block(last_block_hash_or_number)
            .await
            .context("failed fetching last processed block")?
            .context("last_processed block should always exist. Check the chain")?;

        let last_processed = Self {
            block,
            num_key,
            hash_key,
            redis_pool,
        };

        Ok(last_processed)
    }

    fn number(&self) -> Option<u64> {
        self.block.number.as_ref().map(|x| x.as_u64())
    }

    fn hash(&self) -> Option<&H256> {
        self.block.hash.as_ref()
    }

    async fn set(&mut self, new: Block<TxHash>) -> anyhow::Result<()> {
        self.block = new;

        // TODO: save last_processed somewhere external in case this process exits
        if let Some(pool) = self.redis_pool {
            let mut conn = pool.get().await?;

            // TODO: i thought we could use u64.encode_hex(), but nope. just store as decimal
            let num = self.number().unwrap().to_string();
            let hash = self.hash().unwrap().encode_hex();

            trace!(%num, ?hash, "saving");

            // TODO: pipe and set them together atomically
            // TODO: only set if number is > the current number
            // TODO: use json to store them in one key
            let _: Option<String> = conn.set(&self.num_key, num).await?;
            let _: Option<String> = conn.set(&self.hash_key, hash).await?;
        }

        Ok(())
    }
}

async fn binary_search_eth_get_code(
    provider: &EthersProviderWs,
    factory_address: &Address,
    head_block: &U64,
) -> anyhow::Result<U64> {
    let mut low_block_num: U64 = 1.into();
    let mut high_block_num: U64 = *head_block;

    while low_block_num < high_block_num {
        let middle_block_num = (high_block_num + low_block_num) / 2;

        trace!("checking for {:?} @ {}", factory_address, middle_block_num);

        let middle_get_code = provider
            .get_code(*factory_address, Some(middle_block_num.into()))
            .await?;

        // TODO: only bother getting prev_get_code if middle_get_code is not empty
        let prev_get_code = provider
            .get_code(*factory_address, Some((middle_block_num - 1).into()))
            .await?;

        let k = match (prev_get_code.is_empty(), middle_get_code.is_empty()) {
            (true, false) => {
                // the middle block has the code, but the previous block does not. success!
                Ordering::Equal
            }
            (true, true) => {
                // middle block does not have the code
                // prev can't if middle doesn't (at least for our non-selfdestruct contracts)
                Ordering::Less
            }
            (false, false) => {
                // both blocks have the code
                Ordering::Greater
            }
            (false, true) => unimplemented!(),
        };

        match k {
            Ordering::Equal => return Ok(middle_block_num),
            Ordering::Greater => high_block_num = middle_block_num,
            Ordering::Less => low_block_num = middle_block_num + 1,
        }
    }

    Err(anyhow::anyhow!("code not found!"))
}

#[instrument(skip(http_client, factory))]
pub async fn process_block(
    block: &Block<TxHash>,
    http_client: &Client,
    factory: &LlamaNodes_PaymentContracts_Factory<EthersProviderWs>,
    proxy_http_url: &str,
) -> anyhow::Result<Vec<H256>> {
    debug!("processing block");

    let mut processed_txs = vec![];

    for uncle in block.uncles.iter() {
        // TODO: get the uncle data and only post if they pass the log bloom filter
        let r = http_client
            .post(format!("{}/user/balance_uncle/{:?}", proxy_http_url, uncle))
            .send()
            .await?;

        info!(?r, "uncle submitted");

        let j = r.text().await?;

        info!(?j, "uncle submitted");

        // basic DOS protection
        sleep(Duration::from_millis(10)).await;
    }

    // TODO: can we check multiple inputs at the same time?
    let logs_bloom = block
        .logs_bloom
        .context("blocks here should always have a bloom")?;

    let mut payment_received_filter = factory.payment_received_filter();

    // // TODO: this is wrong. fix this
    // let payment_received_topic0 = if let ValueOrArray::Value(Some(x)) =
    //     payment_received_filter.filter.topics[0].as_ref().unwrap()
    // {
    //     x
    // } else {
    //     panic!("topic0 should always be set");
    // };

    // // check the topic bloom first because it has more bits and so hopefully has less common false positives
    // let topics_bloom_input = BloomInput::Hash(payment_received_topic0.as_fixed_bytes());

    // if !logs_bloom.contains_input(topics_bloom_input) {
    //     trace!("block does not contain logs using the payment_received event");
    //     return Ok(processed_txs);
    // }

    // next check the contract address
    let factory_address = factory.address();
    let address_bloom_input = BloomInput::Raw(factory_address.as_bytes());

    if !logs_bloom.contains_input(address_bloom_input) {
        trace!("block does not contain logs using the factory address");
        return Ok(processed_txs);
    }

    // bloom filters will never have a false negative
    // there can be false positives, but bloom filter checks cut out a lot of unnecessary eth_getLogs
    trace!("bloom filters passed");

    let block_hash = block.hash.unwrap();

    // filter for the logs only on this new head block
    payment_received_filter = payment_received_filter.at_block_hash(block_hash);

    // rust-analyzer loses this type
    let logs_with_meta: Vec<(PaymentReceivedFilter, LogMeta)> = payment_received_filter
        .query_with_meta()
        .await
        .context("parsing logs with metadata")?;

    trace!(?logs_with_meta, "from payment received filter");

    for (_, log_meta) in logs_with_meta {
        let txid = log_meta.transaction_hash;

        if log_meta.address != factory_address {
            debug!(?log_meta, ?factory_address, "skipping incorrect factory");
            continue;
        }

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

        processed_txs.push(txid);

        // basic DOS protection
        sleep(Duration::from_millis(10)).await;
    }

    Ok(processed_txs)
}
