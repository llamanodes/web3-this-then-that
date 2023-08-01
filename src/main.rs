use anyhow::Context;
use dotenv::dotenv;
use futures::future::try_join_all;
use sentry::types::Dsn;
use std::env;
use tracing::warn;
use tracing_subscriber::{prelude::*, EnvFilter};
use web3_this_then_that::get_redis_pool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // use dotenv to get config
    let _ = dotenv().ok();

    // parse values from the config
    // optional, but if sentry dsn is set, it MUST parse
    let sentry_dsn = env::var("W3TTT_SENTRY_DSN")
        .ok()
        .map(|x| x.parse::<Dsn>().unwrap());

    let proxy_urls = env::var("W3TTT_PROXY_URLS")
        .context("Setting W3TTT_PROXY_URLS in your environment is required")?;

    // optional
    let redis_url = env::var("W3TTT_REDIS_URL").ok();

    if redis_url.is_none() {
        warn!("W3TTT_REDIS_URL is not set! Last processed block will not survive restarts");
    }

    // set up sentry connection
    let _sentry_guard = sentry::init(sentry::ClientOptions {
        dsn: sentry_dsn,
        release: sentry::release_name!(),
        // Enable capturing of traces
        // we set to 100% here while we develop, but production will likely want to configure this smaller
        traces_sample_rate: 1.0,
        ..Default::default()
    });

    tracing_subscriber::fmt()
        // create a subscriber that uses the RUST_LOG env var for filtering levels
        .with_env_filter(EnvFilter::from_default_env())
        // print a pretty output to the terminal
        .pretty()
        // the root subscriber is ready
        .finish()
        // Register the Sentry tracing layer to capture breadcrumbs, events, and spans
        .with(sentry_tracing::layer())
        // register as the default global subscriber
        .init();

    let http_client = reqwest::Client::new();
    let redis_pool = redis_url.map(get_redis_pool);
    // TODO: prometheus metrics?

    let handles = proxy_urls.split(',').map(|proxy_url| {
        let f = web3_this_then_that::run_forever(
            http_client.clone(),
            proxy_url.to_owned(),
            redis_pool.clone(),
        );

        tokio::spawn(f)
    });

    try_join_all(handles).await?;

    Ok(())
}
