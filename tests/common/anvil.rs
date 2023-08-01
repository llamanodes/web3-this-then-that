// TODO: option to spawn in a dedicated thread?
// TODO: option to subscribe to another anvil and copy blocks

use std::sync::Arc;

use ethers::{
    signers::LocalWallet,
    utils::{Anvil, AnvilInstance},
};
use tracing::info;
use web3_this_then_that::{EthersProviderHttp, EthersProviderWs};

/// on drop, the anvil instance will be shut down
pub struct TestAnvil {
    pub instance: AnvilInstance,
    pub provider: EthersProviderHttp,
}

impl TestAnvil {
    #[allow(unused)]
    pub async fn spawn(chain_id: u64, fork: Option<&str>) -> Self {
        info!(?chain_id);

        // TODO: configurable rpc and block
        let mut instance = Anvil::new().chain_id(chain_id);

        if let Some(fork) = fork {
            instance = instance.fork(fork);
        }

        let instance = instance.spawn();

        let provider = EthersProviderHttp::try_from(instance.endpoint()).unwrap();

        Self { instance, provider }
    }

    #[allow(unused)]
    pub fn wallet(&self, id: usize) -> LocalWallet {
        self.instance.keys()[id].clone().into()
    }

    #[allow(unused)]
    pub async fn ws_provider(&self) -> Arc<EthersProviderWs> {
        let x = EthersProviderWs::connect(self.instance.ws_endpoint())
            .await
            .unwrap();

        Arc::new(x)
    }
}
