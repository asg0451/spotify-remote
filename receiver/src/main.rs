use std::sync::{Arc, RwLock};

use anyhow::Result;

use clap::Parser;

use receiver::{bot::BotOptions, creds_registry::CredsRegistry};

#[derive(Debug, Parser)]
struct Options {
    // TODO: rename me
    #[clap(short, long, default_value = "8080")]
    grpc_port: u16,
    #[clap(flatten)]
    bot_opts: BotOptions,
}

#[tokio::main]
async fn main() -> Result<()> {
    common::util::setup_logging()?;
    let _ = common::util::load_env(".env");

    let opts = Options::parse();

    let stream_registry = Arc::new(RwLock::new(CredsRegistry::default()));

    tracing::info!("starting http server on port {}", opts.grpc_port);
    let rpc_server_jh = {
        let registry = Arc::clone(&stream_registry);
        tokio::spawn(async move {
            let srv = receiver::server::Server::new(registry);
            srv.run(opts.grpc_port).await?;
            Ok::<(), anyhow::Error>(())
        })
    };

    tracing::info!("starting discord bot");

    let disc_jh = tokio::spawn(async move {
        receiver::bot::run_bot(opts.bot_opts, stream_registry).await?;
        Ok::<(), anyhow::Error>(())
    });

    tokio::select! {
        _ = rpc_server_jh => {
            tracing::info!("http server exited");
        }
        _ = disc_jh => {
            tracing::info!("discord client exited");
        }
        _ = common::util::ctrl_c() => {
            tracing::info!("received ctrl-c");
        }
    };

    Ok(())
}
