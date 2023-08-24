# copy everything into a single image
FROM ubuntu:22.04

# set up ENV. do this first because it won't ever change
ENV PATH /llama/venv/bin:$PATH

# Create llama user to avoid running container with root
RUN set -eux; \
    \
    mkdir /llama; \
    adduser --home /llama --shell /sbin/nologin --gecos '' --no-create-home --disabled-password --uid 1000 llama; \
    chown -R llama /llama

# install python dependencies
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    set -eux; \
    \
    apt-get update; \
    apt-get install --no-install-recommends --yes build-essential ca-certificates python3.10-venv python3.10-dev

# Run everything else as the "llama" user, not root
USER llama
# ENV HOME /llama  # TODO: do we need this?
WORKDIR /llama

# create virtualenv
RUN --mount=type=cache,uid=1000,gid=1000,target=/llama/.cache \
    \
    python3 -m venv /llama/venv

# install python dependencies
RUN --mount=type=bind,source=requirements.txt,target=/llama/venv/requirements.txt \
    --mount=type=cache,uid=1000,gid=1000,target=/llama/.cache \
    \
    pip install -r /llama/venv/requirements.txt

# install ape dependencies
RUN --mount=type=bind,source=ape-config.yaml,target=/llama/ape-config.yaml \
    --mount=type=cache,uid=1000,gid=1000,target=/llama/.cache \
    set -eux; \
    \
    ape plugins install .

# TODO: ape caches?

# install app
COPY . /llama/
RUN --mount=type=cache,uid=1000,gid=1000,target=/llama/.cache \
    set -eux; \
    \
    mkdir -p /llama/.ape /llama/.build /llama/.tokenlists; \
    ape compile

VOLUME [ "/llama/.ape", "/llama/.tokenlists" ]
ENTRYPOINT [ "silverback", "run", "entrypoint:app" ]
