FROM rust:1.70.0-bullseye AS builder

WORKDIR /app
ENV CARGO_TERM_COLOR always

# a next-generation test runner for Rust projects.
# We only pay the installation cost once, 
# TODO: do this in a seperate FROM and COPY it in
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-nextest

# foundry is needed to run tests
# TODO: do this in a seperate FROM and COPY it in
ENV PATH /root/.foundry/bin:$PATH
RUN curl -L https://foundry.paradigm.xyz | bash && foundryup

# copy the application
COPY . .

# test the application with cargo-nextest
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo nextest run

# build the application
# using a "release" profile (which install does) is **very** important
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo install \
    --locked \
    --no-default-features \
    --path . \
    --root /usr/local/bin

#
# We do not need the Rust toolchain to run the binary!
#
FROM debian:bullseye-slim AS runtime

# Create llama user to avoid running container with root
RUN mkdir /llama \
    && adduser --home /llama --shell /sbin/nologin --gecos '' --no-create-home --disabled-password --uid 1001 llama \
    && chown -R llama /llama

USER llama

ENTRYPOINT ["web3_proxy_payment_watcher"]
CMD [ "--config", "/config.toml", "daemon" ]

ENV RUST_LOG "warn,web3_proxy_payment_watcher=debug"

COPY --from=builder /usr/local/bin/* /usr/local/bin/
