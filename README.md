# Web3 This Then That

A simple python app to watch the blockchain and take actions when things happen.

## Development

Build the app:

    python3.10 -m venv venv
    . venv/bin/activate
    pip install --upgrade pip wheel
    pip install -r requirements-dev.txt
    ape plugins install .

Update requirements:

    pip-compile -U --output-file requirements-dev.txt ./requirements-dev.in ./requirements.in
    # and then probably run: `pip install -r requirements-dev.txt`

    pip-compile -U ./requirements.in
    # and then maybe run: `pip install -r requirements.txt`

Build a docker image:

    docker build -t llamanodes/web3-this-then-that .
    # TODO: push to ECR?

Run a docker image:

    # TODO: example docker-compose
    docker run \
        --rm -it \
        --env WEB3_LLAMANODES_API_KEY="your ulid from <https://llamanodes.com/dashboard>" \
        --volume ~/.ape:/llama/.ape \
        --volume ~/.tokenlists:/llama/.tokenlists \
        llamanodes/web3-this-then-that

Open an interactive shell inside docker:

    docker run \
        --rm -it \
        --entrypoint bash \
        --env WEB3_LLAMANODES_API_KEY="your ulid from <https://llamanodes.com/dashboard>" \
        --volume ~/.ape:/llama/.ape \
        --volume ~/.tokenlists:/llama/.tokenlists \
        llamanodes/web3-this-then-that

Inside the interactive shell, you can use silverback:

    silverback run "entrypoint:app" --network ethereum
    silverback run "entrypoint:app" --network polygon

Likely useful environment variables:

    SILVERBACK_BROKER_CLASS="taskiq:InMemoryBroker"
    SILVERBACK_ENABLE_METRICS="True"

    SILVERBACK_BROKER_URI=
    SILVERBACK_RESULT_BACKEND_CLASS=
    SILVERBACK_RESULT_BACKEND_URI=

    ARBISCAN_API_KEY=
    BASESCAN_API_KEY=
    BSCSCAN_API_KEY=
    ETHERSCAN_API_KEY=
    FTMSCAN_API_KEY=
    GNOSISSCAN_API_KEY=
    OPTIMISTIC_ETHERSCAN_API_KEY=
    POLYGONSCAN_API_KEY=
    POLYGON_ZKEVM_ETHERSCAN_API_KEY=
    SNOWTRACE_API_KEY=
