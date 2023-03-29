//! Subscribe to a Web3 RPC and do something when an event is seen
//!
//! Initially, this will be hard coded for web3-proxy deposit transactions,
//! but it would probably be useful generally.
//!
//! # Running
//!
//! ```bash
//! W3TTT_RPC_WS_URL=wss://polygon.llamarpc.com cargo run
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
//! - [ ] proper logging
//! - [ ] moka cache for blocks
//! - [ ] petgraph for tracking forks?
//! - [ ] retry rather than exit
//!
//!
//!

// TODO: decide how to subscribe to logs
/*
I've heard multiple reports that subcribing to logs with a websocket occassionaly miss logs. I see a few options.

1. just subscribe to logs and if one gets missed, the user can report their txid/address and we can manually submit the tx
2. simple process
    a. subscribe to new heads
    b. when a block arrives, call eth_getLogs with our topics for every block we haven't seen
3. complex process (only make backend requests if there is a relevant log)
    a. subscribe to new heads
    b. when a block arrives, get the log_bloom for every block we haven't seen
    c. check the bloom filter for any logs we care about
    d. if the bloom filter returns false, skip this block
    e. call eth_getLogs with our topics

if a re-org happens, we should POST the txid
*/

use anyhow::Context;
use dotenv::dotenv;
use ethers::{
    abi::ethereum_types::BloomInput,
    prelude::{abigen, LogMeta},
    providers::{Middleware, Provider, StreamExt, Ws},
    types::{Address, ValueOrArray},
};
use std::{env, sync::Arc};

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

    let rpc_ws_url = env::var("W3TTT_RPC_WS_URL")
        .context("Setting W3TTT_RPC_WS_URL in your environment is required")?;

    let factory_address = env::var("W3TTT_LLAMANODES_FACTORY")
        .unwrap_or("0x4e3BC2054788De923A04936C6ADdB99A05B0Ea36".to_string())
        .parse::<Address>()
        .context("W3TTT_LLAMANODES_FACTORY is not a valid address")?;

    // TODO: prometheus metrics?
    // TODO: dotfile exporter for the chain?

    // TODO: acquire mysql lock? multiple reports for the same event doesn't really hurt anything

    // TODO: create a websocket connection to an rpc provider
    let provider = Arc::new(
        Provider::<Ws>::connect(rpc_ws_url)
            .await
            .context("failed connecting to websocket")?,
    );

    // TODO: query mysql (or maybe redis) to know the last row we processed
    // 40157242 (march 9, 2023) is when 0x4e3bc2054788de923a04936c6addb99a05b0ea36 was created
    // TODO: last_processed should be a number and a hash (or maybe the full block object)
    let mut last_processed = provider
        .get_block(40157242)
        .await
        .context("failed fetching last processed block")?
        .expect("there should always be a block");

    let factory = LlamaNodes_PaymentContracts_Factory::new(factory_address, provider.clone());

    let payment_received_filter = factory.payment_received_filter();

    let payment_received_topic0 = if let ValueOrArray::Value(Some(x)) =
        payment_received_filter.filter.topics[0].as_ref().unwrap()
    {
        x
    } else {
        panic!("topic0 should always be set");
    };

    let mut new_heads_sub = provider.subscribe_blocks().await?;

    while let Some(new_head) = new_heads_sub.next().await {
        // TODO: move this to a seperate function that is spawned
        // TODO: don't just query new_head. work backwards from new_head to last_processed
        // TODO: working backwards will need thought to handle reorgs. will need to find the common ancestor
        let new_head_number = new_head.number.unwrap();
        let new_head_hash = new_head.hash.unwrap();
        let new_head_parent = new_head.parent_hash;

        if new_head_hash == last_processed.hash.unwrap() {
            // we've already seen this block
            continue;
        }

        println!("new_head: {} {:#?}", new_head_number, new_head_hash,);

        if new_head_parent != last_processed.hash.unwrap() {
            // TODO: work backwards from new_head until last_processed is the parent or ancestor of the processed block
        }

        // TODO: can we check multiple inputs at the same time?
        let logs_bloom = new_head.logs_bloom.unwrap();

        // check the topic bloom first because it has more bits and so hopefully has less common false positives
        let topics_bloom_input = BloomInput::Hash(payment_received_topic0.as_fixed_bytes());

        if !logs_bloom.contains_input(topics_bloom_input) {
            last_processed = new_head;
            continue;
        }

        // next check the contract address
        let address_bloom_input = BloomInput::Raw(factory_address.as_bytes());

        if !logs_bloom.contains_input(address_bloom_input) {
            last_processed = new_head;
            continue;
        }

        // there can be false positives, but bloom filter checks cut out a lot of unnecessary eth_getLogs

        // filter for the logs only on this new head block
        let payment_received_filter = factory
            .payment_received_filter()
            .at_block_hash(new_head_hash);

        // rust-analyzer loses this type
        let logs_with_meta: Vec<(PaymentReceivedFilter, LogMeta)> =
            payment_received_filter.query_with_meta().await?;

        // println!("logs with meta: {:#?}", logs_with_meta);

        for (_, log_meta) in logs_with_meta {
            let txid = log_meta.transaction_hash;

            // TODO: submit the txid to a new endpoint on web3-proxy
        }

        last_processed = new_head;
    }

    Ok(())
}
