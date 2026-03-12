use tracing::level_filters::LevelFilter;
use tracing_subscriber::{Layer, filter::Targets, layer::SubscriberExt, registry::Registry};

use crate::config::LoggerConfig;

pub fn init_ocypode_logger(config: &LoggerConfig) {
    let mut layers = Vec::new();

    let targets = Targets::new().with_default(config.default_level);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(config.with_thread_name)
        .with_filter(targets)
        .boxed();
    layers.push(fmt_layer);

    // Isolate the console subscriber server in a dedicated OS thread and runtime.
    // This ensures the monitoring server remains accessible for debugging even if
    // the main Tokio runtime experiences heavy load, starvation, or deadlocks.
    if config.enable_tokio_console {
        let (layer, server) =
            console_subscriber::ConsoleLayer::builder().with_default_env().build();

        let console_layer = layer
            .with_filter(
                Targets::new()
                    .with_target("tokio", LevelFilter::TRACE)
                    .with_target("runtime", LevelFilter::TRACE),
            )
            .boxed();

        layers.push(console_layer);

        std::thread::Builder::new()
            .name(format!("{} tokio-console", config.name))
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio-console runtime")
                    .block_on(async move {
                        println!("serving console subscriber");
                        if let Err(e) = server.serve().await {
                            eprintln!("console subscriber server error: {}", e);
                        }
                    });
            })
            .expect("failed to spawn console subscriber thread");
    }

    let subscriber = Registry::default().with(layers);

    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set global tracing subscriber");
}
