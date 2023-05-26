use anyhow::Result;

pub fn setup_logging() -> Result<()> {
    use tracing_subscriber::filter::EnvFilter;
    use tracing_subscriber::prelude::*;

    // TODO: how to make this automatic again. need some kind of macro prob
    // let us = env!("CARGO_PKG_NAME").replace('-', "_");
    // .add_directive(format!("{us}=trace").parse()?);

    let mut regular_filter = EnvFilter::from_default_env();
    if let Err(_) = std::env::var(EnvFilter::DEFAULT_ENV) {
        regular_filter = regular_filter
            .add_directive("warn".parse()?)
            .add_directive("songbird=debug".parse()?)
            .add_directive("common=trace".parse()?)
            .add_directive("forwarder=trace".parse()?)
            .add_directive("receiver=trace".parse()?)
            .add_directive("player=trace".parse()?);
    }

    // NOTE: listens at 12.0.0.1:6669
    let console_layer = console_subscriber::spawn();
    let registry = tracing_subscriber::registry().with(console_layer);

    if !atty::is(atty::Stream::Stderr) {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
                    .with_target(false)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_filter(regular_filter),
            )
            .init();
    } else {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_filter(regular_filter),
            )
            .init();
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

#[cfg(unix)]
pub async fn ctrl_c() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut int = signal(SignalKind::interrupt()).unwrap();
    let mut term = signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = int.recv() => {}
        _ = term.recv() => {}
    };
}

#[cfg(unix)]
pub async fn ctrl_c_and_pipe() {
    use tokio::signal::unix::{signal, SignalKind};
    let others = ctrl_c();
    let mut pipe = signal(SignalKind::pipe()).unwrap();
    tokio::select! {
        _ = others => {}
        _ = pipe.recv() => {}
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_logging() {
        setup_logging().unwrap();
        tracing::info!("test");
    }
}
