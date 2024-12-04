use tracing_subscriber::{EnvFilter, fmt};

/// Initialize tracing with custom format
pub fn setup_tracing() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info")); // Default to "info" if RUST_LOG is not set

    fmt()
        .with_env_filter(env_filter)
        .without_time()
        .with_target(false)
        .init();

    Ok(())
}
