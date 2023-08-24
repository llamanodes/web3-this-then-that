from ape import chain
from ape.api import BlockAPI
from asyncio import sleep
from ape.types import ContractLog
from silverback import SilverBackApp
from taskiq import TaskiqState
from web3.utils.address import to_checksum_address
import requests
from urllib3.util import Retry
from requests import Session
from requests.adapters import HTTPAdapter
from ape.logging import logger, LogLevel

s = Session()
retries = Retry(
    backoff_factor=0.1,
    backoff_jitter=0.1,
    status_forcelist=[413, 429, 500, 502, 503, 504],
)
s.mount("https://", HTTPAdapter(max_retries=retries))

# initialize the app
app = SilverBackApp()

logger.set_level(LogLevel.INFO)
# TODO: sentry

logger.info("Starting app...")

ecosystem = app.network_manager.ecosystem.name

# TODO: get this from app.provider_manager or something like that
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
    return {
        "message": "Starting..."
    }


# TODO: use websocket fork instead of new_block_timeout
@app.on_(
    PaymentFactory.PaymentReceived,
    new_block_timeout=new_block_timeout,
    start_block=factory_deploy_block,
)
def exec_payment_received(log: ContractLog):
    if log.contract_address == factory_address:
        try:
            s.post(f"{txid_url}/{log.transaction_hash}")
        except Exception as e:
            logger.error("Error: %s", e)

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
