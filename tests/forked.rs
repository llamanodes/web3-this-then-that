mod common;

use std::time::Duration;

use anyhow::Context;
use common::TestAnvil;
use ethers::{
    providers::Middleware,
    types::{Address, H256},
};
use web3_this_then_that::{process_block, EthersProviderWs, LlamaNodes_PaymentContracts_Factory};

async fn test_deposit_transaction(
    p: &EthersProviderWs,
    c: &reqwest::Client,
    f: &web3_this_then_that::LlamaNodes_PaymentContracts_Factory<
        ethers::providers::Provider<ethers::providers::Ws>,
    >,
    block_num: &u64,
    proxy_http_url: &str,
) -> anyhow::Result<Vec<H256>> {
    let block = p
        .get_block(*block_num)
        .await?
        .context(format!("no block for {}", block_num))?;

    process_block(&block, c, f, proxy_http_url).await
}

#[test_log::test(tokio::test)]
async fn test_deposit_txs() {
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    // TODO: actual parametetrized tests
    let fixtures = [
        (
            1,
            17778849,
            vec!["0xb490f9b7f9eb4ce681b4ce5abe5becac7bf05170c264481a178127c1b45820a7"],
        ),
        (137, 44450755, vec![]),
        (
            137,
            45741168,
            vec!["0x075737b315ca257cd92f5c3ea68a59ab788589ad4555283c750d057506d9f2c7"],
        ),
    ];

    for (chain_id, block_num, txs) in fixtures {
        let fork = match chain_id {
            1 => format!("https://ethereum.llamarpc.com@{block_num}"),
            137 => format!("https://polygon.llamarpc.com@{block_num}"),
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

        let x = test_deposit_transaction(&ws_provider, &c, &factory, &block_num, &proxy_http_url)
            .await
            .unwrap();

        let txs: Vec<H256> = txs.into_iter().map(|x| x.parse().unwrap()).collect();

        assert_eq!(
            x, txs,
            "wrong number of transaction is supposed to be submitted"
        );
    }
}
