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

#[cfg_attr(not(feature = "tests-needing-llamanodes"), ignore)]
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
        (
            1,
            17780186,
            vec!["0x53df23dcc525e6ae573cbfc5f052e218827a2ac58f59fc43cb9a59e2217cc2b6"],
        ),
        (
            1,
            17780267,
            vec!["0x481e96ce00a8a753b946a09c31e00d6fbd14ade373981313f462e17aca5f58d4"],
        ),
        (
            1,
            17784495,
            vec!["0xf3535e6bbc5aec16eb348f96de0da03ce84d84743118aebb19b2fad741298da1"],
        ),
        (
            1,
            17788123,
            vec!["0x5ff3998eeb092d66c1a6835e61e0b34c6095014dfbdc9038502b1bd8f3da0129"],
        ),
        (
            1,
            17790954,
            vec!["0x0dc995dd0fe73f8ab52a38985176d56255e54410cdf1c715acc004fb0371feb6"],
        ),
        (
            1,
            17803594,
            vec!["0x4a176a210bf25b68b97dc987c90c48bf4150e5895e6820efe5dfb911589554fd"],
        ),
        (
            1,
            17813470,
            vec!["0x7c4b52c7a2ab1fa4f3207cb3ea50916d713bdde15e42baf172d996bfd338b5f0"],
        ),
        (137, 44450755, vec![]),
        (
            137,
            44786848,
            vec!["0xfb56d67cb7c89a3fdd486a6af32c9b4731366dbd62ec17053d78ba74e48bf5e2"],
        ),
        (
            137,
            44834034,
            vec!["0x49c26c6481a7e88e3b135eb5a1fd6f61334fc4b4f5518779d6e1c6d953ee1321"],
        ),
        (
            137,
            44834392,
            vec!["0x1acf38fb07550ec8844f7257355487a975ef6f09c49f079f4eee62d60ef85bec"],
        ),
        (
            137,
            44905574,
            vec!["0x0d4e7de3fad3ef23f018e8f81feed3ffffe4268bd4c6ad28bbc666ada7068874"],
        ),
        (
            137,
            45538814,
            vec!["0xa825327af81d9d9fe6ba279179bfc6d342b29540475c811a3d4f59928a9cdd46"],
        ),
        (
            137,
            45542541,
            vec!["0x326a9d202b6d38614fcd29ef3377a98b0ffb5ae9b91a4aea4c916532ebb6b6cc"],
        ),
        (
            137,
            45578296,
            vec!["0x4dde12ac56beb06d442b55696d9f620b542b24f88f138e7eacf7edf11ece8f7b"],
        ),
        (
            137,
            45589233,
            vec!["0x7332d0dee79fd3e30cf4ebe1a1ed19187b1ac88cbec007d82aebaffdae5923da"],
        ),
        (
            137,
            45598700,
            vec!["0xc250b7accaf059819ae788e605e41ab7eaa10ac71d4ffb930e797e516dbf3f29"],
        ),
        (
            137,
            45663114,
            vec!["0xd45f3ac9cbc46aa87c60501ed71eefc036f04cce9c010a3a435226f7b60442fd"],
        ),
        (
            137,
            45713313,
            vec!["0xb2e2e8178fe51a05ba25a546df564d56d9d42f1414707f15be7cac0579fa64da"],
        ),
        (
            137,
            45741168,
            vec!["0x075737b315ca257cd92f5c3ea68a59ab788589ad4555283c750d057506d9f2c7"],
        ),
        (
            137,
            45753308,
            vec!["0xcb473ed655f646c488301a42fda3b4811a3b0f07c1153e7af4864a0488eef53c"],
        ),
    ];

    for (chain_id, block_num, txs) in fixtures {
        // TODO: fork urls from the env (with llamarpc defaults)
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
