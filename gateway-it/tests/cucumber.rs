use axum::{routing::get, Router};
use cucumber::{given, then, when, World};
use gateway_cp::RunningServer;
use gateway_dp::GatewayDpConfig;
use serde_json::json;
use std::net::{SocketAddr, TcpListener};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use std::{path::PathBuf, time::SystemTime};

#[derive(Default, World)]
#[world(init = Self::new)]
struct TestWorld {
    server: Option<RunningServer>,
    upstream_addr: Option<SocketAddr>,
    dp_bind: Option<String>,
    dp_handle: Option<thread::JoinHandle<()>>,
    client: reqwest::Client,
    last_status: Option<u16>,
    last_body: Option<serde_json::Value>,
    last_text: Option<String>,
    log_path: Option<String>,
}

impl std::fmt::Debug for TestWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestWorld")
            .field("last_status", &self.last_status)
            .field("last_body", &self.last_body)
            .finish()
    }
}

impl TestWorld {
    async fn new() -> Result<Self, anyhow::Error> {
        let log_path = init_test_logging();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1000))
            .build()?;
        Ok(Self { client, log_path: Some(log_path), ..Self::default() })
    }
}

#[given("the control plane is running")]
async fn control_plane_running(world: &mut TestWorld) {
    if world.server.is_some() {
        return;
    }

    let server = gateway_cp::start_for_test()
        .await
        .expect("failed to start control plane");
    world.server = Some(server);
}

#[given("an upstream service is running")]
async fn upstream_running(world: &mut TestWorld) {
    if world.upstream_addr.is_some() {
        return;
    }

    let app = Router::new().route("/v1/users", get(|| async { "upstream-ok" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    wait_for_tcp(&addr.to_string(), Duration::from_secs(5));
    world.upstream_addr = Some(addr);
}

#[when(expr = "I register a policy with id {string} and version {string}")]
async fn register_policy(world: &mut TestWorld, id: String, version: String) {
    let server = world.server.as_ref().expect("server not running");
    let body = json!({
        "id": id,
        "version": version,
        "wasm_uri": "file:///policies/authn.wasm",
        "sha256": "deadbeef",
        "config": { "mode": "jwt", "issuer": "example" }
    });

    let response = world
        .client
        .post(format!("{}/policies", server.base_url))
        .json(&body)
        .send()
        .await
        .expect("failed to call /policies");

    world.last_status = Some(response.status().as_u16());
    world.last_body = response.json().await.ok();
}

#[when(expr = "I create a route with id {string}")]
async fn create_route(world: &mut TestWorld, id: String) {
    let server = world.server.as_ref().expect("server not running");
    let body = json!({
        "id": id,
        "match": { "path_prefix": "/v1/users", "method": ["GET", "POST"] },
        "lb": "round_robin",
        "failover": { "enabled": true, "max_failovers": 1, "retry_on": ["connect_failure", "5xx"], "per_try_timeout_ms": 1000 },
        "upstreams": [
            { "url": "http://10.0.0.12:8080", "weight": 100, "priority": 0 }
        ],
        "policies": [
            { "stage": "pre_route", "id": "authn", "version": "1.0.0" }
        ]
    });

    let response = world
        .client
        .post(format!("{}/routes", server.base_url))
        .json(&body)
        .send()
        .await
        .expect("failed to call /routes");

    world.last_status = Some(response.status().as_u16());
    world.last_body = response.json().await.ok();
}

#[when("I create a route pointing to the upstream")]
async fn create_route_to_upstream(world: &mut TestWorld) {
    let server = world.server.as_ref().expect("server not running");
    let upstream = world.upstream_addr.expect("upstream not running");
    let body = json!({
        "id": "users",
        "match": { "path_prefix": "/v1/users", "method": ["GET"] },
        "upstreams": [
            { "url": format!("http://{upstream}") }
        ],
        "policies": []
    });

    let response = world
        .client
        .post(format!("{}/routes", server.base_url))
        .json(&body)
        .send()
        .await
        .expect("failed to call /routes");

    world.last_status = Some(response.status().as_u16());
    world.last_body = response.json().await.ok();
}

#[given("the gateway is running")]
#[when("the gateway is started")]
async fn start_gateway(world: &mut TestWorld) {
    if world.dp_bind.is_some() {
        return;
    }
    let server = world.server.as_ref().expect("server not running");
    let port = pick_port();
    let bind = format!("127.0.0.1:{port}");

    let dp_config = GatewayDpConfig {
        listener: gateway_dp::config::ListenerConfig { bind: bind.clone(), tls: None },
        control_plane: gateway_dp::config::ControlPlaneConfig {
            grpc_endpoint: server.grpc_url.clone(),
            tls: None,
        },
        logging: gateway_dp::config::LoggingConfig {
            level: "info".to_string(),
            json: true,
            rolling_file: None,
        },
        limits: gateway_dp::config::LimitsConfig {
            max_body_bytes: 10 * 1024 * 1024,
            pre_upstream_body_bytes: 64 * 1024,
        },
    };

    let handle = thread::spawn(move || {
        gateway_dp::run(dp_config);
    });

    world.dp_bind = Some(bind);
    world.dp_handle = Some(handle);
    wait_for_tcp(&world.dp_bind.clone().unwrap(), Duration::from_secs(5));
    if let Some(handle) = world.dp_handle.as_ref() {
        if handle.is_finished() {
            panic!("gateway thread exited unexpectedly");
        }
    }
}

#[when("I list routes")]
async fn list_routes(world: &mut TestWorld) {
    let server = world.server.as_ref().expect("server not running");
    let response = world
        .client
        .get(format!("{}/routes", server.base_url))
        .send()
        .await
        .expect("failed to call /routes");

    world.last_status = Some(response.status().as_u16());
    world.last_body = response.json().await.ok();
}

#[when(expr = "I wait for the route {string} to be available")]
async fn wait_for_route(world: &mut TestWorld, path: String) {
    let bind = world.dp_bind.clone().expect("gateway not started");
    let url = format!("http://{bind}{path}");
    let mut last_status = 0;
    let mut last_err = String::new();

    for _ in 0..30 {
        match world.client.get(&url).send().await {
            Ok(resp) => {
                last_status = resp.status().as_u16();
                if last_status == 200 {
                    return;
                }
            }
            Err(err) => {
                last_err = format!("{err:?}");
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let log_hint = world
        .log_path
        .as_ref()
        .map(|path| format!(" (logs: {path})"))
        .unwrap_or_default();
    panic!(
        "route did not become available, last status {last_status}, last error {last_err}{log_hint}"
    );
}

#[then(expr = "a request to {string} should return {string}")]
async fn request_should_return(world: &mut TestWorld, path: String, expected: String) {
    let bind = world.dp_bind.clone().expect("gateway not started");
    let url = format!("http://{bind}{path}");
    let response = world
        .client
        .get(&url)
        .send()
        .await
        .expect("failed to call gateway");

    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    world.last_text = Some(text.clone());
    let log_hint = world
        .log_path
        .as_ref()
        .map(|path| format!(" (logs: {path})"))
        .unwrap_or_default();
    assert_eq!(
        status,
        200,
        "expected 200, got {status} with body {text}{log_hint}"
    );
    assert_eq!(text, expected, "unexpected response body");
}

#[then(expr = "the response status should be {int}")]
async fn assert_status(world: &mut TestWorld, status: u16) {
    let actual = world.last_status.unwrap_or_default();
    assert_eq!(actual, status, "expected status {status}, got {actual}");
}

#[then(expr = "the response should include route {string}")]
async fn assert_route_present(world: &mut TestWorld, route_id: String) {
    let body = world.last_body.clone().expect("no response body");
    let routes = body.as_array().expect("expected array response");

    let found = routes.iter().any(|route| {
        route.get("id").and_then(|id| id.as_str()) == Some(route_id.as_str())
    });

    assert!(found, "route {route_id} not found in response");
}

#[tokio::main]
async fn main() {
    TestWorld::run("./features").await;
}

fn pick_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().unwrap().port()
}

fn wait_for_tcp(bind: &str, timeout: Duration) {
    let addr = bind.parse::<SocketAddr>().expect("parse bind");
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("gateway did not start listening on {bind}");
}

fn init_test_logging() -> String {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }

    if let Ok(existing) = std::env::var("GATEWAY_LOG_PATH") {
        return existing;
    }

    let dir = PathBuf::from("target/test-logs");
    let _ = std::fs::create_dir_all(&dir);

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = format!("gateway-it-{}-{}.log", std::process::id(), nanos);
    let path = dir.join(file_name);

    std::env::set_var("GATEWAY_LOG_PATH", path.to_string_lossy().to_string());
    path.to_string_lossy().to_string()
}
