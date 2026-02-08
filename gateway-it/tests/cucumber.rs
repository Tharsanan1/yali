use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use reqwest::Method;
use std::time::Duration;

#[derive(Default, World)]
#[world(init = Self::new)]
struct TestWorld {
    cp_base: String,
    dp_base: String,
    upstream_url: String,
    upstream_check_url: Option<String>,
    client: reqwest::Client,
    last_status: Option<u16>,
    last_body: Option<serde_json::Value>,
    last_text: Option<String>,
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
        let cp_base = std::env::var("GATEWAY_IT_CP_BASE_URL")
            .map_err(|_| anyhow::anyhow!("GATEWAY_IT_CP_BASE_URL is required"))?;
        let dp_base = std::env::var("GATEWAY_IT_DP_BASE_URL")
            .map_err(|_| anyhow::anyhow!("GATEWAY_IT_DP_BASE_URL is required"))?;
        let upstream_url = std::env::var("GATEWAY_IT_UPSTREAM_URL")
            .map_err(|_| anyhow::anyhow!("GATEWAY_IT_UPSTREAM_URL is required"))?;
        let upstream_check_url = std::env::var("GATEWAY_IT_UPSTREAM_CHECK_URL").ok();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(3000))
            .build()?;

        Ok(Self {
            cp_base,
            dp_base,
            upstream_url,
            upstream_check_url,
            client,
            ..Self::default()
        })
    }
}

#[given("the control plane is running")]
async fn control_plane_running(world: &mut TestWorld) {
    let url = format!("{}/health", world.cp_base);
    wait_for_http_ok(&world.client, &url, Duration::from_secs(10)).await;
}

#[given("the gateway is running")]
async fn gateway_running(world: &mut TestWorld) {
    let url = format!("{}", world.dp_base);
    wait_for_http_ready(&world.client, &url, Duration::from_secs(10)).await;
}

#[given("an upstream service is running")]
async fn upstream_running(world: &mut TestWorld) {
    if let Some(check) = &world.upstream_check_url {
        wait_for_http_ok(&world.client, check, Duration::from_secs(10)).await;
    }
}

#[when(expr = "I {word} {string} on the control plane")]
async fn request_on_control_plane(world: &mut TestWorld, method: String, path: String) {
    let cp_base = world.cp_base.clone();
    send_request(world, &cp_base, &method, &path, None).await;
}

#[when(expr = "I {word} {string} on the control plane with JSON:")]
async fn request_on_control_plane_with_json(
    world: &mut TestWorld,
    method: String,
    path: String,
    #[step] step: &Step,
) {
    let body = json_from_docstring(world, step);
    let cp_base = world.cp_base.clone();
    send_request(world, &cp_base, &method, &path, Some(body)).await;
}

#[when(expr = "I {word} {string} on the gateway")]
async fn request_on_gateway(world: &mut TestWorld, method: String, path: String) {
    let dp_base = world.dp_base.clone();
    send_request(world, &dp_base, &method, &path, None).await;
}

#[when(expr = "I {word} {string} on the gateway with JSON:")]
async fn request_on_gateway_with_json(
    world: &mut TestWorld,
    method: String,
    path: String,
    #[step] step: &Step,
) {
    let body = json_from_docstring(world, step);
    let dp_base = world.dp_base.clone();
    send_request(world, &dp_base, &method, &path, Some(body)).await;
}

#[when(expr = "I wait for the route {string} to be available")]
async fn wait_for_route(world: &mut TestWorld, path: String) {
    let url = format!("{}{}", world.dp_base, path);
    wait_for_http_ok(&world.client, &url, Duration::from_secs(30)).await;
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
    raw.replace("{{upstream_url}}", &world.upstream_url)
        .replace("{{control_plane}}", &world.cp_base)
        .replace("{{gateway}}", &world.dp_base)
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

async fn wait_for_http_ok(client: &reqwest::Client, url: &str, timeout: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("endpoint not ready: {url}");
}

async fn wait_for_http_ready(client: &reqwest::Client, url: &str, timeout: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if client.get(url).send().await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("endpoint not reachable: {url}");
}
