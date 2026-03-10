fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let subscriber = tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(false)
        .with_env_filter(filter)
        .json()
        .finish();

    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        error!(error = %e, "failed to init tracing");
    }
}
