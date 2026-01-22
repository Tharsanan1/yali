# Yali - AI-Native Gateway

A high-performance API Gateway optimized for AI workloads, built on Cloudflare's [Pingora](https://github.com/cloudflare/pingora) framework.

> **Note:** This project is under active development. See [TODO.md](./TODO.md) for implementation progress and roadmap.

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

## Prerequisites

- Rust 1.70 or later
- Cargo

## Quick Start

### 1. Clone and Build

```bash
git clone https://github.com/Tharsanan1/yali.git
cd yali

# Build in debug mode (faster compilation)
cargo build

# Or build in release mode (optimized)
cargo build --release
```

### 2. Create Configuration

Create a `config.json` file with your gateway configuration:

```json
{
  "providers": [
    {
      "id": "provider_openai",
      "name": "OpenAI Production",
      "spec": {
        "type": "openai",
        "endpoint": "https://api.openai.com",
        "adapter": {
          "auth": {
            "type": "bearer",
            "secret_ref": "env://OPENAI_API_KEY"
          },
          "url": {
            "path_prefix": "/v1/chat/completions"
          }
        }
      }
    }
  ],
  "backends": [
    {
      "id": "backend_main",
      "name": "Main Backend",
      "spec": {
        "load_balancing": {
          "algorithm": "failover"
        },
        "providers": [
          {
            "ref": "provider_openai",
            "priority": 1,
            "weight": 100
          }
        ]
      }
    }
  ],
  "routes": [
    {
      "id": "route_chat",
      "spec": {
        "match": {
          "path": "/v1/chat",
          "type": "prefix",
          "methods": ["POST"]
        },
        "backend_ref": "backend_main"
      }
    }
  ]
}
```

### 3. Set Environment Variables

```bash
# Required: Set your OpenAI API key (or other provider keys)
export OPENAI_API_KEY=sk-your-api-key-here

# Optional: Configure gateway listen address (default: 0.0.0.0:8080)
export GATEWAY_LISTEN_ADDR=127.0.0.1:8080

# Optional: Set log level (default: info)
export RUST_LOG=info
```

### 4. Run the Gateway

```bash
# Using debug build
./target/debug/yali-gateway config.json

# Or using release build
./target/release/yali-gateway config.json
```

You should see output like:
```
INFO yali_gateway: Starting AI-Native Gateway (Yali)
INFO yali_gateway: Configuration loaded providers=1 backends=1 routes=1
INFO yali_gateway: Starting HTTP proxy service addr=127.0.0.1:8080
INFO yali_gateway: Gateway is running on 127.0.0.1:8080
```

### 5. Send Requests

Now you can send requests through the gateway:

```bash
# Send a chat completion request
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ],
    "temperature": 0.7
  }'
```

## Configuration Guide

### Provider Configuration

A **Provider** represents a single AI endpoint:

```json
{
  "id": "unique_provider_id",
  "name": "Human-readable name",
  "spec": {
    "type": "openai",
    "endpoint": "https://api.openai.com",
    "adapter": {
      "auth": {
        "type": "bearer",
        "secret_ref": "env://OPENAI_API_KEY"
      },
      "url": {
        "path_prefix": "/v1/chat/completions"
      },
      "headers": {
        "add": {"Custom-Header": "value"},
        "remove": ["Unwanted-Header"]
      }
    }
  }
}
```

#### Authentication Types

| Type | Description | Example |
|------|-------------|---------|
| `bearer` | Authorization: Bearer token | `{"type": "bearer", "secret_ref": "env://API_KEY"}` |
| `header` | Custom header with key | `{"type": "header", "key": "x-api-key", "secret_ref": "env://API_KEY"}` |
| `none` | No authentication | `{"type": "none"}` |

#### Secret References

| Format | Description |
|--------|-------------|
| `env://VAR_NAME` | Read from environment variable |

### Backend Configuration

A **Backend** groups providers and applies policies:

```json
{
  "id": "backend_id",
  "name": "Backend Name",
  "spec": {
    "load_balancing": {
      "algorithm": "failover"
    },
    "providers": [
      {"ref": "provider_primary", "priority": 1, "weight": 100},
      {"ref": "provider_fallback", "priority": 2, "weight": 0}
    ],
    "timeout": {
      "connect": "5s",
      "response": "120s"
    },
    "retry": {
      "attempts": 3,
      "conditions": ["5xx", "connect-failure"]
    }
  }
}
```

### Route Configuration

A **Route** defines traffic entry points:

```json
{
  "id": "route_id",
  "host": "optional-tenant.example.com",
  "spec": {
    "match": {
      "path": "/v1/chat",
      "type": "prefix",
      "methods": ["POST"]
    },
    "backend_ref": "backend_id"
  }
}
```

#### Match Types

| Type | Description | Example |
|------|-------------|---------|
| `prefix` | Matches path prefix | `/v1/chat` matches `/v1/chat/completions` |
| `exact` | Matches exact path | `/health` matches only `/health` |

## Testing

### Run All Tests

```bash
cargo test
```

### Run Integration Tests Only

```bash
cargo test --test integration_test
```

### Run with Verbose Output

```bash
cargo test -- --nocapture
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GATEWAY_LISTEN_ADDR` | Address to listen on | `0.0.0.0:8080` |
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | `info` |

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

## Project Structure

```
yali/
├── src/
│   ├── main.rs          # Binary entry point
│   ├── lib.rs           # Library exports
│   ├── config/
│   │   └── mod.rs       # Configuration structs (Provider, Backend, Route)
│   ├── state.rs         # Gateway state and routing
│   └── proxy.rs         # Pingora proxy implementation
├── tests/
│   └── integration_test.rs  # End-to-end tests
├── config.example.json  # Example configuration
├── TODO.md              # Development roadmap
├── prd                  # Product Requirements Document
└── Cargo.toml
```

## Development

See [TODO.md](./TODO.md) for the full development roadmap and current progress.

### Building for Development

```bash
# Fast compilation for development
cargo build

# Run tests continuously
cargo watch -x test
```

### Building for Production

```bash
cargo build --release
```

## License

MIT

