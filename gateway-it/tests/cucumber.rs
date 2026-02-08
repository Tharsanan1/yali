use axum::{routing::get, Router};
use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use gateway_cp::RunningServer;
use gateway_dp::GatewayDpConfig;
use reqwest::Method;
use std::net::{SocketAddr, TcpListener, TcpStream};
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

#[when(expr = "I {word} {string} on the control plane")]
async fn request_on_control_plane(world: &mut TestWorld, method: String, path: String) {
    let base = control_plane_base(world);
    send_request(world, &base, &method, &path, None).await;
}

#[when(expr = "I {word} {string} on the control plane with JSON:")]
async fn request_on_control_plane_with_json(
    world: &mut TestWorld,
    method: String,
    path: String,
    #[step] step: &Step,
) {
    let base = control_plane_base(world);
    let body = json_from_docstring(world, step);
    send_request(world, &base, &method, &path, Some(body)).await;
}

#[when(expr = "I {word} {string} on the gateway")]
async fn request_on_gateway(world: &mut TestWorld, method: String, path: String) {
    let base = gateway_base(world);
    send_request(world, &base, &method, &path, None).await;
}

#[when(expr = "I {word} {string} on the gateway with JSON:")]
async fn request_on_gateway_with_json(
    world: &mut TestWorld,
    method: String,
    path: String,
    #[step] step: &Step,
) {
    let base = gateway_base(world);
    let body = json_from_docstring(world, step);
    send_request(world, &base, &method, &path, Some(body)).await;
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

#[then(expr = "the response status should be {int}")]
async fn assert_status(world: &mut TestWorld, status: u16) {
    let actual = world.last_status.unwrap_or_default();
    assert_eq!(actual, status, "expected status {status}, got {actual}");
}

#[then(expr = "the response text should be {string}")]
async fn assert_text(world: &mut TestWorld, expected: String) {
    let text = world.last_text.clone().unwrap_or_default();
    assert_eq!(text, expected, "unexpected response body");
}

#[then("the JSON response should include:")]
async fn assert_json_includes(world: &mut TestWorld, #[step] step: &Step) {
    let expected = json_from_docstring(world, step);
    let actual = world
        .last_body
        .clone()
        .or_else(|| {
            world
                .last_text
                .as_ref()
                .and_then(|text| serde_json::from_str(text).ok())
        })
        .expect("no JSON response captured");

    assert!(
        json_contains(&actual, &expected),
        "expected JSON to include {expected}, got {actual}"
    );
}

#[then("the JSON response should equal:")]
async fn assert_json_equals(world: &mut TestWorld, #[step] step: &Step) {
    let expected = json_from_docstring(world, step);
    let actual = world
        .last_body
        .clone()
        .or_else(|| {
            world
                .last_text
                .as_ref()
                .and_then(|text| serde_json::from_str(text).ok())
        })
        .expect("no JSON response captured");

    assert_eq!(actual, expected, "unexpected JSON response");
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

fn control_plane_base(world: &TestWorld) -> String {
    world
        .server
        .as_ref()
        .expect("control plane not running")
        .base_url
        .clone()
}

fn gateway_base(world: &TestWorld) -> String {
    let bind = world.dp_bind.clone().expect("gateway not started");
    format!("http://{bind}")
}

async fn send_request(
    world: &mut TestWorld,
    base_url: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) {
    let url = format!("{base_url}{path}");
    let method = Method::from_bytes(method.to_uppercase().as_bytes())
        .unwrap_or_else(|_| panic!("invalid HTTP method: {method}"));
    let mut request = world.client.request(method, &url);
    if let Some(json_body) = body {
        request = request.json(&json_body);
    }

    let response = request.send().await.expect("request failed");
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_text = Some(text.clone());
    world.last_body = serde_json::from_str(&text).ok();
}

fn json_from_docstring(world: &TestWorld, step: &Step) -> serde_json::Value {
    let raw = step
        .docstring
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| panic!("step is missing a JSON docstring: {}", step.value));
    let rendered = render_docstring(world, raw);
    serde_json::from_str(&rendered)
        .unwrap_or_else(|err| panic!("invalid JSON docstring: {err}\n{rendered}"))
}

fn render_docstring(world: &TestWorld, raw: &str) -> String {
    let mut rendered = raw.to_string();
    if let Some(upstream) = world.upstream_addr {
        rendered = rendered.replace("{{upstream}}", &upstream.to_string());
        rendered = rendered.replace("{{upstream_url}}", &format!("http://{upstream}"));
    }
    if let Some(server) = world.server.as_ref() {
        rendered = rendered.replace("{{control_plane}}", &server.base_url);
        rendered = rendered.replace("{{control_plane_grpc}}", &server.grpc_url);
    }
    if let Some(bind) = world.dp_bind.as_ref() {
        rendered = rendered.replace("{{gateway}}", &format!("http://{bind}"));
    }
    rendered
}

fn json_contains(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (actual, expected) {
        (serde_json::Value::Array(actual_arr), serde_json::Value::Object(_)) => {
            actual_arr.iter().any(|item| json_contains(item, expected))
        }
        (serde_json::Value::Array(actual_arr), serde_json::Value::Array(expected_arr)) => {
            expected_arr.iter().all(|expected_item| {
                actual_arr
                    .iter()
                    .any(|actual_item| json_contains(actual_item, expected_item))
            })
        }
        (serde_json::Value::Object(actual_obj), serde_json::Value::Object(expected_obj)) => {
            expected_obj.iter().all(|(key, expected_val)| {
                actual_obj
                    .get(key)
                    .map(|actual_val| json_contains(actual_val, expected_val))
                    .unwrap_or(false)
            })
        }
        _ => actual == expected,
    }
}
