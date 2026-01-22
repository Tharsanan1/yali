# Yali - AI-Native Gateway

A high-performance API Gateway optimized for AI workloads, built on Cloudflare's [Pingora](https://github.com/cloudflare/pingora) framework.

## Features

- **Three-tier Resource Model**: Clean separation of concerns with Provider, Backend, and Route resources
- **Protocol Adaptation**: Transform requests between different AI provider formats
- **Load Balancing**: Support for failover, round-robin, and weighted algorithms
- **Resilience**: Circuit breakers, retries with exponential backoff, and health checks
- **Multi-tenant**: Subdomain-based tenant isolation with radix trie routing

## Architecture

```
┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐
│     Provider     │     │      Backend     │     │      Route       │
│  (The Endpoint)  │◄────│  (The Policy)    │◄────│   (The Entry)    │
├──────────────────┤     ├──────────────────┤     ├──────────────────┤
│ • endpoint URL   │     │ • load_balancing │     │ • path match     │
│ • auth adapter   │     │ • circuit_breaker│     │ • filters        │
│ • headers        │     │ • health_check   │     │ • backend_ref    │
│ • url_rewrite    │     │ • retry policy   │     └──────────────────┘
│ • protocol type  │     │ • timeout        │
└──────────────────┘     │ • provider_refs[]│
                         └──────────────────┘
```

## Quick Start

### Building

```bash
cargo build --release
```

### Running

```bash
# Set your API key
export OPENAI_API_KEY=sk-...

# Run the gateway
./target/release/yali-gateway config.json
```

### Configuration

Create a `config.json` file:

```json
{
  "providers": [{
    "id": "provider_openai",
    "name": "OpenAI",
    "spec": {
      "type": "openai",
      "endpoint": "https://api.openai.com",
      "adapter": {
        "auth": { "type": "bearer", "secret_ref": "env://OPENAI_API_KEY" },
        "url": { "path_prefix": "/v1/chat/completions" }
      }
    }
  }],
  "backends": [{
    "id": "backend_main",
    "name": "Main",
    "spec": {
      "providers": [{ "ref": "provider_openai" }]
    }
  }],
  "routes": [{
    "id": "route_chat",
    "spec": {
      "match": { "path": "/v1/chat", "type": "prefix", "methods": ["POST"] },
      "backend_ref": "backend_main"
    }
  }]
}
```

### Testing

```bash
# Run all tests
cargo test

# Run integration tests only
cargo test --test integration_test
```

### Making a Request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GATEWAY_LISTEN_ADDR` | Address to listen on | `0.0.0.0:8080` |
| `RUST_LOG` | Log level | `info` |

## Provider Types

| Type | Endpoint Pattern | Default Auth |
|------|------------------|--------------|
| `openai` | `api.openai.com` | Bearer token |
| `azure_openai` | `*.api.cognitive.microsoft.com` | `api-key` header |
| `anthropic` | `api.anthropic.com` | `x-api-key` header |
| `google_ai` | `generativelanguage.googleapis.com` | Bearer token |
| `custom` | Any URL | Configurable |

## Load Balancing Algorithms

| Algorithm | Behavior |
|-----------|----------|
| `failover` | Uses highest priority provider; falls back on failure |
| `round_robin` | Distributes evenly across healthy providers |
| `weighted` | Distributes based on weight values |

## License

MIT
