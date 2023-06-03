use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
struct Options {
    #[clap(
        short = 'a',
        long,
        env,
        help = "address of the receiver server, eg http://localhost:8080"
    )]
    receiver_addr: String,
    #[clap(
        short = 'n',
        long,
        default_value = "danube",
        env,
        help = "name of the device"
    )]
    device_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    common::util::setup_logging()?;

    let opts = Options::parse();

    let forwarder = forwarder::Forwarder::new(opts.receiver_addr, opts.device_name).await?;

    forwarder.run().await?;

    Ok(())
}
