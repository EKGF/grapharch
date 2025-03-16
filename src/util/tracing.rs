use tracing_subscriber::{EnvFilter, fmt};

/// Initialize tracing with custom format
pub fn setup_tracing(
    verbosity: clap_verbosity_flag::Verbosity,
) -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        match verbosity.log_level() {
            Some(level) => EnvFilter::new(level.as_str()),
            None => EnvFilter::new("fatal"),
        }
    });

    fmt()
        .with_env_filter(env_filter)
        .without_time()
        .with_target(false)
        .init();

    Ok(())
}
