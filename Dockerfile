FROM rust:1.70.0-bullseye AS rust_builder

WORKDIR /app
ENV CARGO_UNSTABLE_SPARSE_REGISTRY true
ENV CARGO_TERM_COLOR always
ENV PATH "/root/.foundry/bin:/root/.cargo/bin:${PATH}"

# nextest runs tests in parallel (done its in own FROM so that it can run in parallel)
FROM rust_builder as rust_nextest
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    set -eux; \
    \
    cargo install --locked cargo-nextest

# foundry/anvil are needed to run tests (done its in own FROM so that it can run in parallel)
FROM rust_builder as rust_foundry
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    set -eux; \
    \
    curl -L https://foundry.paradigm.xyz | bash && /root/.foundry/bin/foundryup

FROM rust_builder as rust_with_env

# changing our features doesn't change any of the steps above
ENV WEB3_PROXY_FEATURES ""

# copy the app
COPY . .

# fetch deps
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    set -eux; \
    \
    cargo --locked --verbose fetch

# build tests (done its in own FROM so that it can run in parallel)
FROM rust_with_env as build_tests

COPY --from=rust_foundry /root/.foundry/bin/anvil /root/.foundry/bin/anvil
COPY --from=rust_nextest /usr/local/cargo/bin/cargo-nextest* /usr/local/cargo/bin/

# test the application with cargo-nextest
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    set -eux; \
    \
    ls -la /root/.foundry/bin/anvil; \
    echo $PATH; \
    which anvil; \
    RUST_LOG=web3_proxy=trace,info \
    cargo \
    --frozen \
    --offline \
    --verbose \
    nextest run \
    --features "$WEB3_PROXY_FEATURES" --no-default-features \
    ; \
    touch /test_success

FROM rust_with_env as build_app

# # build the release application
# # using a "release" profile (which install does by default) is **very** important
# # TODO: use the "faster_release" profile which builds with `codegen-units = 1` (but compile is SLOW)
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    set -eux; \
    \
    cargo install \
    --features "$WEB3_PROXY_FEATURES" \
    --frozen \
    --no-default-features \
    --offline \
    --path . \
    --root /usr/local \
    ;

# copy this file so that docker actually creates the build_tests container
# without this, the runtime container doesn't need build_tests and so docker build skips it
COPY --from=build_tests /test_success /

#
# We do not need the Rust toolchain or any deps to run the binary!
#
FROM debian:bullseye-slim AS runtime

# Create llama user to avoid running container with root
RUN set -eux; \
    \
    mkdir /llama; \
    adduser --home /llama --shell /sbin/nologin --gecos '' --no-create-home --disabled-password --uid 1001 llama; \
    chown -R llama /llama

USER llama

ENTRYPOINT ["web3-this-then-that"]

ENV RUST_BACKTRACE "1"
ENV RUST_LOG "warn,web3_this_then_that=debug"

COPY --from=build_app /usr/local/bin/* /usr/local/bin/
