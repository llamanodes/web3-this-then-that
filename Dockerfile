FROM rust:1.70.0-bullseye AS builder

WORKDIR /app
ENV CARGO_TERM_COLOR always
ENV PATH /root/.foundry/bin:$PATH

# a next-generation test runner for Rust projects.
# We only pay the installation cost once, 
# TODO: do this in a seperate FROM and COPY it in
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    \
    cargo install cargo-nextest

# foundry is needed to run tests
# TODO: do this in a seperate FROM and COPY it in
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    \
    curl -L https://foundry.paradigm.xyz | bash && foundryup

FROM builder as build_tests

# test the application with cargo-nextest
RUN --mount=type=bind,target=.,rw \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target,sharing=private \
    \
    cargo nextest run && \
    touch /test_success

FROM builder as build_app

# build the application
# using a "release" profile (which install does) is **very** important
RUN --mount=type=bind,target=.,rw \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target,sharing=private \
    \
    cargo install \
    --locked \
    --no-default-features \
    --path . \
    --root /usr/local/bin

# copy this file so that docker actually creates the build_tests container
# without this, the runtime container doesn't need build_tests and so docker build skips it
COPY --from=build_tests /test_success /

#
# We do not need the Rust toolchain to run the binary!
#
FROM debian:bullseye-slim AS runtime

# Create llama user to avoid running container with root
RUN mkdir /llama \
    && adduser --home /llama --shell /sbin/nologin --gecos '' --no-create-home --disabled-password --uid 1001 llama \
    && chown -R llama /llama

USER llama

ENTRYPOINT ["web3_this_then_that"]

ENV RUST_LOG "warn,web3_this_then_that=debug"

COPY --from=build_app /usr/local/bin/* /usr/local/bin/
