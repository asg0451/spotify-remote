use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
struct Options {
    #[clap(short, long, default_value = "https://sproter.coldcutz.net")]
    receiver_addr: String,
    #[clap(short, long, default_value = "danube")]
    device_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    common::util::setup_logging()?;

    let opts = Options::parse();

    let forwarder =
        forwarder::server::Transmitter::new(opts.receiver_addr, opts.device_name).await?;

    forwarder.run().await?;

    Ok(())
}
