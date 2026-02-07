use cucumber::{given, then, when, World};
use gateway_cp::RunningServer;
use serde_json::json;

#[derive(Default, World)]
#[world(init = Self::new)]
struct TestWorld {
    server: Option<RunningServer>,
    client: reqwest::Client,
    last_status: Option<u16>,
    last_body: Option<serde_json::Value>,
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
        Ok(Self { client: reqwest::Client::new(), ..Self::default() })
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
