use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};
use tracing_subscriber::Layer;

pub fn init(level: &str, json: bool) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let writer: BoxMakeWriter = if let Ok(path) = std::env::var("GATEWAY_LOG_PATH") {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("gateway.log");
        let file_appender = tracing_appender::rolling::never(dir, file);
        BoxMakeWriter::new(std::io::stdout.and(file_appender))
    } else {
        BoxMakeWriter::new(std::io::stdout)
    };

    let fmt_layer = if json {
        fmt::layer().json().with_target(true).with_writer(writer).boxed()
    } else {
        fmt::layer().with_target(true).with_writer(writer).boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .ok();
}
