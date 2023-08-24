# Web3 This Then That

A simple python app to watch the blockchain and take actions when things happen.

## Development

Build the app:

    python3.10 -m venv venv
    . venv/bin/activate
    pip install --upgrade pip wheel -c ./requirements-dev.txt
    pip install -r requirements-dev.txt
    ape plugins install .

Update requirements:

    pip-compile -U --output-file requirements-dev.txt --resolver=backtracking ./requirements-dev.in ./requirements.in
    # and then probably run: `pip install -r requirements-dev.txt`

    pip-compile -U --resolver=backtracking ./requirements.in
    # and then maybe run: `pip install -r requirements.txt`

Build a docker image:

    docker build -t llamanodes/web3-this-then-that .
    # TODO: push to ECR?

Run a docker image:

    # TODO: example docker-compose
    docker run \
        --rm -it \
        --env LLAMA_RPC_KEY="your ulid from <https://llamanodes.com/dashboard>" \
        --volume ~/.ape:/llama/.ape \
        --volume ~/.tokenlists:/llama/.tokenlists \
        llamanodes/web3-this-then-that

Open an interactive shell inside docker:

    docker run \
        --rm -it \
        --entrypoint bash \
        --env LLAMA_RPC_KEY="your ulid from <https://llamanodes.com/dashboard>" \
        --volume ~/.ape:/llama/.ape \
        --volume ~/.tokenlists:/llama/.tokenlists \
        llamanodes/web3-this-then-that

Inside the interactive shell, you can use silverback:

    silverback run "entrypoint:app" --network :mainnet

Likely useful environment variables:

    SILVERBACK_BROKER_CLASS="taskiq:InMemoryBroker"
    SILVERBACK_BROKER_URI=""
    SILVERBACK_ENABLE_METRICS="False"
    SILVERBACK_RESULT_BACKEND_CLASS=""
    SILVERBACK_RESULT_BACKEND_URI=""
