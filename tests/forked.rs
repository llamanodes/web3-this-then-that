mod common;

use std::time::Duration;

use anyhow::Context;
use common::TestAnvil;
use ethers::{
    providers::Middleware,
    types::{Address, H256},
};
use tracing::info;
use web3_this_then_that::{process_block, EthersProviderWs, LlamaNodes_PaymentContracts_Factory};

async fn test_deposit_transaction(
    p: &EthersProviderWs,
    c: &reqwest::Client,
    f: &web3_this_then_that::LlamaNodes_PaymentContracts_Factory<
        ethers::providers::Provider<ethers::providers::Ws>,
    >,
    txid: &str,
    proxy_http_url: &str,
) -> anyhow::Result<usize> {
    let receipt = p
        .get_transaction_receipt(txid.parse::<H256>().unwrap())
        .await?
        .context("no transaction receipt")?;

    info!(?receipt);

    let block_hash = receipt.block_hash.unwrap();

    let block = p
        .get_block(block_hash)
        .await?
        .context(format!("no block for {}", block_hash))?;

    process_block(&block, c, f, proxy_http_url).await
}

#[test_log::test(tokio::test)]
async fn test_deposit_txs() {
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    // TODO: actual parametetrized tests
    let txs = [
        (
            1,
            17778849,
            "0xb490f9b7f9eb4ce681b4ce5abe5becac7bf05170c264481a178127c1b45820a7",
        ),
        (
            137,
            45741168,
            "0x075737b315ca257cd92f5c3ea68a59ab788589ad4555283c750d057506d9f2c7",
        ),
    ];

    for (chain_id, block, tx) in txs {
        let fork = match chain_id {
            1 => format!("https://ethereum.llamarpc.com@{block}"),
            137 => format!("https://polygon.llamarpc.com@{block}"),
            _ => unimplemented!("unknown chain: {}", chain_id),
        };

        let factory_address: Address = "0x216ce6e49e2e713e41383ba4c5d84a0d36189640"
            .parse()
            .unwrap();

        let a = TestAnvil::spawn(chain_id, Some(fork.as_str())).await;

        let proxy_http_url = a.instance.endpoint();
        let ws_provider = a.ws_provider().await;

        let factory =
            LlamaNodes_PaymentContracts_Factory::new(factory_address, ws_provider.clone());

        let x = test_deposit_transaction(&ws_provider, &c, &factory, tx, &proxy_http_url)
            .await
            .unwrap();

        assert_eq!(x, 1, "1 transaction is supposed to be submitted");
    }
}
