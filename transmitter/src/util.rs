use anyhow::Result;

pub fn setup_logging() -> Result<()> {
    use tracing_subscriber::{filter::EnvFilter, fmt};

    // // forward log events to tracing
    // tracing_log::LogTracer::init()?;

    let us = env!("CARGO_PKG_NAME").replace('-', "_");

    let filter = EnvFilter::from_default_env()
        .add_directive("warn".parse()?)
        .add_directive(format!("{us}=trace").parse()?);

    // .json().with_current_span(true)

    let b = fmt().with_env_filter(filter).with_writer(std::io::stderr);

    if !atty::is(atty::Stream::Stderr) {
        b.json()
            .with_current_span(true)
            .with_span_list(true)
            .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_level(true)
            .init();
    } else {
        b.init();
    }

    Ok(())
}

pub fn load_env(path: &str) -> Result<()> {
    let contents = std::fs::read_to_string(path)?;
    contents
        .trim_end()
        .split('\n')
        .filter(|s| !s.starts_with('#'))
        .map(|l| l.split('=').collect())
        .for_each(|a: Vec<&str>| {
            let (k, v) = (a[0], a[1]);
            std::env::set_var(k, v);
        });
    tracing::info!("set env vars from {}", path);
    Ok(())
}
