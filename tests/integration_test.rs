//! Integration tests for the AI-Native Gateway.
//!
//! This test module validates the end-to-end functionality of the gateway
//! by starting a mock LLM service, configuring the gateway, and sending
//! requests through it.

use reqwest::Client;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// Find an available port for testing
fn find_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port");
    listener.local_addr().unwrap().port()
}

/// Start a mock LLM service that returns OpenAI-compatible responses
async fn start_mock_llm_service(port: u16) -> tokio::task::JoinHandle<()> {
    use axum::{http::StatusCode, routing::post, Json, Router};

    async fn chat_completions_handler(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
        // Extract the model from the request
        let model = payload
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("gpt-4");

        // Create an OpenAI-compatible response
        let response = json!({
            "id": "chatcmpl-test123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! I'm a mock LLM response. How can I help you today?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 15,
                "total_tokens": 25
            }
        });

        (StatusCode::OK, Json(response))
    }

    let app = Router::new().route("/v1/chat/completions", post(chat_completions_handler));

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to bind mock LLM service");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    })
}

/// Create the gateway configuration JSON pointing to the mock service
fn create_gateway_config(mock_llm_port: u16) -> String {
    let config = json!({
        "providers": [{
            "id": "provider_mock_llm",
            "name": "Mock LLM Provider",
            "spec": {
                "type": "openai",
                "endpoint": format!("http://127.0.0.1:{}", mock_llm_port),
                "adapter": {
                    "auth": {
                        "type": "bearer",
                        "secret_ref": "env://MOCK_API_KEY"
                    },
                    "url": {
                        "path_prefix": "/v1/chat/completions"
                    }
                }
            }
        }],
        "backends": [{
            "id": "backend_mock",
            "name": "Mock Backend",
            "spec": {
                "load_balancing": {
                    "algorithm": "failover"
                },
                "providers": [{
                    "ref": "provider_mock_llm",
                    "priority": 1,
                    "weight": 100
                }]
            }
        }],
        "routes": [{
            "id": "route_chat",
            "spec": {
                "match": {
                    "path": "/v1/chat",
                    "type": "prefix",
                    "methods": ["POST"]
                },
                "backend_ref": "backend_mock"
            }
        }]
    });

    serde_json::to_string_pretty(&config).unwrap()
}

/// Start the gateway as a subprocess
fn start_gateway_process(config_path: &str, gateway_port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_yali-gateway"))
        .arg(config_path)
        .env("GATEWAY_LISTEN_ADDR", format!("127.0.0.1:{}", gateway_port))
        .env("MOCK_API_KEY", "test-api-key-12345")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start gateway process")
}

/// Wait for a port to be ready
async fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
        sleep(Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test]
async fn test_gateway_chat_completion_integration() {
    // Find available ports
    let mock_llm_port = find_available_port();
    let gateway_port = find_available_port();

    // Start the mock LLM service
    let _mock_handle = start_mock_llm_service(mock_llm_port).await;

    // Wait for mock service to be ready
    assert!(
        wait_for_port(mock_llm_port, Duration::from_secs(5)).await,
        "Mock LLM service failed to start"
    );

    // Create config file
    let config_content = create_gateway_config(mock_llm_port);
    let temp_dir = std::env::temp_dir();
    let config_path = temp_dir.join(format!("gateway_test_config_{}.json", gateway_port));
    let config_path_str = config_path.to_string_lossy().to_string();
    std::fs::write(&config_path, &config_content).expect("Failed to write config file");

    // Start the gateway process
    let mut gateway_process = start_gateway_process(&config_path_str, gateway_port);

    // Wait for gateway to be ready
    let gateway_ready = wait_for_port(gateway_port, Duration::from_secs(10)).await;

    if !gateway_ready {
        gateway_process.kill().ok();
        gateway_process.wait().ok(); // Wait to avoid zombie process
        std::fs::remove_file(&config_path).ok();
        panic!("Gateway failed to start within timeout");
    }

    // Create HTTP client
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");

    // Send a chat completion request through the gateway
    let request_body = json!({
        "model": "gpt-4",
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant."
            },
            {
                "role": "user",
                "content": "Hello!"
            }
        ],
        "temperature": 0.7
    });

    let response = client
        .post(format!(
            "http://127.0.0.1:{}/v1/chat/completions",
            gateway_port
        ))
        .json(&request_body)
        .send()
        .await;

    // Clean up gateway process
    gateway_process.kill().ok();
    gateway_process.wait().ok(); // Wait to avoid zombie process
    std::fs::remove_file(&config_path).ok();

    let response = response.expect("Failed to send request");

    // Assert HTTP 200
    assert_eq!(
        response.status().as_u16(),
        200,
        "Expected HTTP 200, got {}",
        response.status()
    );

    // Parse and validate response body
    let response_body: Value = response
        .json()
        .await
        .expect("Failed to parse response body");

    // Validate response structure
    assert!(
        response_body.get("id").is_some(),
        "Response should have 'id' field"
    );
    assert!(
        response_body.get("object").is_some(),
        "Response should have 'object' field"
    );
    assert_eq!(
        response_body.get("object").and_then(|v| v.as_str()),
        Some("chat.completion"),
        "Response object should be 'chat.completion'"
    );
    assert!(
        response_body.get("choices").is_some(),
        "Response should have 'choices' field"
    );

    let choices = response_body.get("choices").and_then(|v| v.as_array());
    assert!(choices.is_some(), "Choices should be an array");
    assert!(!choices.unwrap().is_empty(), "Choices should not be empty");

    let first_choice = &choices.unwrap()[0];
    assert!(
        first_choice.get("message").is_some(),
        "Choice should have 'message' field"
    );

    let message = first_choice.get("message").unwrap();
    assert_eq!(
        message.get("role").and_then(|v| v.as_str()),
        Some("assistant"),
        "Message role should be 'assistant'"
    );
    assert!(
        message.get("content").is_some(),
        "Message should have 'content' field"
    );

    // Validate usage information
    assert!(
        response_body.get("usage").is_some(),
        "Response should have 'usage' field"
    );
    let usage = response_body.get("usage").unwrap();
    assert!(
        usage.get("prompt_tokens").is_some(),
        "Usage should have 'prompt_tokens'"
    );
    assert!(
        usage.get("completion_tokens").is_some(),
        "Usage should have 'completion_tokens'"
    );
    assert!(
        usage.get("total_tokens").is_some(),
        "Usage should have 'total_tokens'"
    );

    println!("Integration test passed! Gateway successfully proxied chat completion request.");
}

#[tokio::test]
async fn test_gateway_route_not_found() {
    // Find available ports
    let mock_llm_port = find_available_port();
    let gateway_port = find_available_port();

    // Start the mock LLM service
    let _mock_handle = start_mock_llm_service(mock_llm_port).await;
    assert!(
        wait_for_port(mock_llm_port, Duration::from_secs(5)).await,
        "Mock LLM service failed to start"
    );

    // Create config file
    let config_content = create_gateway_config(mock_llm_port);
    let temp_dir = std::env::temp_dir();
    let config_path = temp_dir.join(format!("gateway_test_config_404_{}.json", gateway_port));
    let config_path_str = config_path.to_string_lossy().to_string();
    std::fs::write(&config_path, &config_content).expect("Failed to write config file");

    // Start the gateway process
    let mut gateway_process = start_gateway_process(&config_path_str, gateway_port);

    // Wait for gateway to be ready
    let gateway_ready = wait_for_port(gateway_port, Duration::from_secs(10)).await;

    if !gateway_ready {
        gateway_process.kill().ok();
        gateway_process.wait().ok(); // Wait to avoid zombie process
        std::fs::remove_file(&config_path).ok();
        panic!("Gateway failed to start within timeout");
    }

    // Create HTTP client
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");

    // Send a request to a non-existent route
    let response = client
        .get(format!(
            "http://127.0.0.1:{}/nonexistent/path",
            gateway_port
        ))
        .send()
        .await;

    // Clean up gateway process
    gateway_process.kill().ok();
    gateway_process.wait().ok(); // Wait to avoid zombie process
    std::fs::remove_file(&config_path).ok();

    let response = response.expect("Failed to send request");

    // Assert HTTP 404
    assert_eq!(
        response.status().as_u16(),
        404,
        "Expected HTTP 404 for non-existent route, got {}",
        response.status()
    );

    // Validate error response
    let response_body: Value = response
        .json()
        .await
        .expect("Failed to parse response body");
    assert!(
        response_body.get("error").is_some(),
        "Response should have 'error' field"
    );

    let error = response_body.get("error").unwrap();
    assert_eq!(
        error.get("code").and_then(|v| v.as_str()),
        Some("ROUTE_NOT_FOUND"),
        "Error code should be 'ROUTE_NOT_FOUND'"
    );

    println!("Route not found test passed!");
}
