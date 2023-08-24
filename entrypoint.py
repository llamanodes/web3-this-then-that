from time import sleep

from ape import chain
from ape.api import BlockAPI
from ape.logging import logger, LogLevel
from ape.types import ContractLog
from asyncio import sleep as async_sleep
from requests.models import Response as Response
from requests_ratelimiter import LimiterSession
from silverback import SilverBackApp
from taskiq import TaskiqState
from urllib3.util import Retry
from urllib3.util.retry import Retry as Retry
from web3.utils.address import to_checksum_address
import requests


# set up an http client with rate limits and retries
# TODO: multiple chains on one host is going to be a problem with rate limits!
# TODO: need an async request library with rate limiting and retries
s = LimiterSession(per_minute=10)

# initialize the app
app = SilverBackApp()

logger.set_level(LogLevel.INFO)
# TODO: sentry

logger.info("Starting app...")

ecosystem = app.network_manager.ecosystem.name

w3p_url = f"https://{ecosystem}.llamarpc.com"

status_url = f"{w3p_url}/status"
txid_url = f"{w3p_url}/user/balance"

logger.info("Waiting for factory address...")

factory_address = None
while not factory_address:
    status = requests.get(status_url).json()

    factory_address = status.get("payment_factory_address")

    if not factory_address:
        sleep(5)

factory_address = to_checksum_address(factory_address)

logger.info("factory_address: %s", factory_address)

PaymentFactory = app.project_manager.get_contract("PaymentFactory").at(factory_address)

# TODO: do a binary search of eth_getCode instead of get_creation_receipt?
# TODO: check redis to know the last block we processed
factory_deploy_block = app.chain_manager.contracts.get_creation_receipt(
    factory_address
).block_number

payment_received_start_block = factory_deploy_block

logger.info("payment_received_start_block: %s", payment_received_start_block)

new_block_timeout = min(max(20, app.network_manager.network.block_time * 10), 60)

logger.info("new_block_timeout: %s", new_block_timeout)


# This is how we trigger off of new blocks
@app.on_(chain.blocks)
def exec_block(block: BlockAPI):
    return {
        "block_hash": block.hash,
    }


# Can handle some stuff on worker startup, like loading a heavy model or something
@app.on_startup()
def startup(state: TaskiqState):
    # TODO: include version number
    return {"message": "Starting..."}


# TODO: use websocket fork instead of new_block_timeout
@app.on_(
    PaymentFactory.PaymentReceived,
    new_block_timeout=new_block_timeout,
    start_block=factory_deploy_block,
)
async def exec_payment_received(log: ContractLog):
    if log.contract_address == factory_address:
        post_url = f"{txid_url}/{log.transaction_hash}"

        # TODO: infinite loop here seems dangerous, but the request session has limits so we shouldn't DOS anything
        while True:
            try:
                x = s.post(post_url)

                x.raise_for_status()

                x = x.json()

                if x.get("result"):
                    # it worked!
                    break

                error = x.get("error")

                logger.warning("POST %s: %s", post_url, error)

                # 'error': {'code': 429, 'message': 'too many requests from 3.17.58.153. Retry in 43 seconds'}}

                if error["code"] == 429:
                    error_data = error.get("data")

                    if error_data:
                        retry_after = error_data.get("retry_after", 60)
                else:
                    retry_after = 10

            except Exception as e:
                logger.error("POST %s: %s", post_url, e)
                retry_after = 10

            # slow down!
            logger.warning("Retrying %s in %s seconds", post_url, retry_after)
            async_sleep(retry_after)

        logger.info("Successful POST %s: %s", post_url, x)

    return {
        "account": log.account,
        "amount": log.amount,
        "contract_address": log.contract_address,
        "token": to_checksum_address(log.token),
        "txid": log.transaction_hash,
    }


# Just in case you need to release some resources or something
@app.on_shutdown()
def shutdown(state: TaskiqState):
    # TODO: include version number
    return {
        "message": "Stopping...",
    }
