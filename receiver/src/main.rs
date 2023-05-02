use std::sync::{Arc, RwLock};

use anyhow::Result;

use clap::Parser;

use receiver::{bot::BotOptions, creds_registry::CredsRegistry};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
struct Options {
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

    let cancel = CancellationToken::new();

    let stream_registry = Arc::new(RwLock::new(CredsRegistry::default()));
    let (tx_pe, rx_pe) = tokio::sync::mpsc::channel(100);

    tracing::info!("starting http server on port {}", opts.grpc_port);
    let rpc_server_cancel = cancel.child_token();
    let mut rpc_server_jh = {
        let creds_registry = Arc::clone(&stream_registry);
        tokio::spawn(async move {
            let app_state = receiver::server::AppState::new(creds_registry, tx_pe);
            receiver::server::run(app_state, opts.grpc_port, rpc_server_cancel).await?;
            Ok::<(), anyhow::Error>(())
        })
    };

    tracing::info!("starting discord bot");

    let bot_cancel = cancel.child_token();
    let mut disc_jh = tokio::spawn(async move {
        receiver::bot::run_bot(opts.bot_opts, stream_registry, rx_pe, bot_cancel.clone()).await?;
        Ok::<(), anyhow::Error>(())
    });

    tokio::select! {
        _ = &mut rpc_server_jh => {
            tracing::info!("http server exited");
        }
        _ = &mut disc_jh => {
            tracing::info!("discord client exited");
        }
        _ = common::util::ctrl_c() => {
            tracing::info!("received ctrl-c");
            cancel.cancel();
            rpc_server_jh.await??;
            disc_jh.await??;
        }
    };

    Ok(())
}
