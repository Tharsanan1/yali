pub mod api;
pub mod config;
pub mod db;
pub mod grpc;
pub mod model;

use api::AppState;
use config::GatewayCpConfig;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};
use tracing_subscriber::Layer;
use tonic::transport::Server as GrpcServer;

pub struct RunningServer {
    pub base_url: String,
    pub grpc_url: String,
    shutdown: tokio::sync::oneshot::Sender<()>,
    handle: tokio::task::JoinHandle<()>,
    grpc_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl RunningServer {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.handle.await;
        let _ = self.grpc_handle.await;
    }
}

pub async fn run(config: GatewayCpConfig) -> Result<(), Box<dyn std::error::Error>> {
    let (listener, state) = build_state_and_listener(&config, Some(config.bind.as_str())).await?;
    let grpc_addr: SocketAddr = config.grpc_bind.parse()?;
    let grpc_listener = TcpListener::bind(grpc_addr).await?;
    let grpc_state = state.config_state.clone();

    let grpc = tokio::spawn(async move {
        let server = grpc_state.server();
        GrpcServer::builder()
            .add_service(server)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(grpc_listener))
            .await
    });

    serve(listener, state).await?;
    let _ = grpc.await?;
    Ok(())
}

pub async fn start_for_test() -> Result<RunningServer, Box<dyn std::error::Error>> {
    let db_suffix = test_db_suffix();
    let config = GatewayCpConfig {
        bind: "127.0.0.1:0".to_string(),
        grpc_bind: "127.0.0.1:0".to_string(),
        logging: config::LoggingConfig { level: "info".to_string(), json: true },
        database_url: format!("sqlite://target/gateway-cp-test-{db_suffix}.db"),
    };

    reset_test_db(&config.database_url)?;

    let (listener, state) = build_state_and_listener(&config, Some(&config.bind)).await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let grpc_state = state.config_state.clone();
    let app = api::router(state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let grpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    let grpc_addr = grpc_listener.local_addr()?;
    let grpc_handle = tokio::spawn(async move {
        let server = grpc_state.server();
        GrpcServer::builder()
            .add_service(server)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(grpc_listener))
            .await
    });

    Ok(RunningServer {
        base_url: format!("http://{}", addr),
        grpc_url: format!("http://{}", grpc_addr),
        shutdown: shutdown_tx,
        handle,
        grpc_handle,
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

fn test_db_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}", std::process::id(), nanos)
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
    let config_state = std::sync::Arc::new(grpc::ConfigState::new());
    config_state.publish_from_db(&pool).await?;
    tracing::info!(db = %database_url, "gateway-cp database ready");

    let bind_addr = bind_override.unwrap_or(&config.bind);
    let listener = TcpListener::bind(bind_addr).await?;

    Ok((listener, AppState { pool, config_state }))
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
