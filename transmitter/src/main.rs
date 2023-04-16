use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
struct Options {
    #[clap(short, long, default_value = "https://sproter.beagle-chickadee.ts.net")]
    receiver_addr: String,
    #[clap(short, long, default_value = "danube")]
    device_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    forwarder::util::setup_logging()?;

    let opts = Options::parse();

    let transmitter =
        forwarder::transmitter::Transmitter::new(opts.receiver_addr, opts.device_name).await?;

    transmitter.run().await?;

    Ok(())
}
