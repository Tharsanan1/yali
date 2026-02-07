pub mod api;
pub mod config;
pub mod db;
pub mod model;

use api::AppState;
use config::GatewayCpConfig;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub struct RunningServer {
    pub base_url: String,
    shutdown: tokio::sync::oneshot::Sender<()>,
    handle: tokio::task::JoinHandle<()>,
}

impl RunningServer {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.handle.await;
    }
}

pub async fn run(config: GatewayCpConfig) -> Result<(), Box<dyn std::error::Error>> {
    let (listener, state) = build_state_and_listener(&config, Some(config.bind.as_str())).await?;
    serve(listener, state).await?;
    Ok(())
}

pub async fn start_for_test() -> Result<RunningServer, Box<dyn std::error::Error>> {
    let config = GatewayCpConfig {
        bind: "127.0.0.1:0".to_string(),
        grpc_bind: "127.0.0.1:0".to_string(),
        logging: config::LoggingConfig { level: "info".to_string(), json: true },
        database_url: "sqlite://target/gateway-cp-test.db".to_string(),
    };

    reset_test_db(&config.database_url)?;

    let (listener, state) = build_state_and_listener(&config, Some(&config.bind)).await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let app = api::router(state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok(RunningServer {
        base_url: format!("http://{}", addr),
        shutdown: shutdown_tx,
        handle,
    })
}

fn reset_test_db(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    const PREFIX: &str = "sqlite://";
    if !url.starts_with(PREFIX) {
        return Ok(());
    }

    let path = &url[PREFIX.len()..];
    if path == ":memory:" {
        return Ok(());
    }

    let file_path = std::path::Path::new(path);
    if file_path.exists() {
        std::fs::remove_file(file_path)?;
    }

    Ok(())
}

pub async fn build_state_and_listener(
    config: &GatewayCpConfig,
    bind_override: Option<&str>,
) -> Result<(TcpListener, AppState), Box<dyn std::error::Error>> {
    init_logging(&config.logging.level, config.logging.json);

    let database_url = normalize_sqlite_url(&config.database_url)?;
    ensure_sqlite_path(&database_url)?;

    let pool = db::connect(&database_url).await?;
    db::init(&pool).await?;

    let bind_addr = bind_override.unwrap_or(&config.bind);
    let listener = TcpListener::bind(bind_addr).await?;

    Ok((listener, AppState { pool }))
}

pub async fn serve(listener: TcpListener, state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let app = api::router(state);
    let addr: SocketAddr = listener.local_addr()?;
    tracing::info!(bind = %addr, "gateway-cp listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_logging(level: &str, json: bool) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let fmt_layer = if json {
        fmt::layer().json().with_target(true).boxed()
    } else {
        fmt::layer().with_target(true).boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .ok();
}

fn normalize_sqlite_url(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    const PREFIX: &str = "sqlite://";
    if !url.starts_with(PREFIX) {
        return Ok(url.to_string());
    }

    let path = &url[PREFIX.len()..];
    if path.is_empty() {
        return Err("sqlite url missing path".into());
    }

    if path.starts_with('/') {
        return Ok(url.to_string());
    }

    let cwd = std::env::current_dir()?;
    let abs = cwd.join(path);
    Ok(format!("{PREFIX}{}", abs.display()))
}

fn ensure_sqlite_path(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    const PREFIX: &str = "sqlite://";
    if !url.starts_with(PREFIX) {
        return Ok(());
    }

    let path = &url[PREFIX.len()..];
    if path == ":memory:" {
        return Ok(());
    }

    let file_path = std::path::Path::new(path);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::OpenOptions::new().create(true).write(true).open(file_path)?;

    Ok(())
}
