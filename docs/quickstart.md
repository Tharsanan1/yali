# Quickstart Guide

Get the Yali AI Gateway up and running in under 5 minutes!

## Prerequisites

- Rust 1.70 or later
- Cargo (Rust's package manager)

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/Tharsanan1/yali.git
cd yali

# Build in release mode
cargo build --release
```

The binary will be available at `./target/release/yali-gateway`.

## Basic Configuration

Create a configuration file named `config.json`:

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

## Environment Variables

Set up the required environment variables:

```bash
# Required: Your OpenAI API key
export OPENAI_API_KEY=sk-your-api-key-here

# Optional: Gateway listen address (default: 0.0.0.0:8080)
export GATEWAY_LISTEN_ADDR=127.0.0.1:8080

# Optional: Log level (trace, debug, info, warn, error)
export RUST_LOG=info
```

## Running the Gateway

```bash
# Run with your configuration file
./target/release/yali-gateway config.json
```

You should see output like:
```
INFO yali_gateway: Starting AI-Native Gateway (Yali)
INFO yali_gateway: Configuration loaded providers=1 backends=1 routes=1
INFO yali_gateway: Starting HTTP proxy service addr=127.0.0.1:8080
INFO yali_gateway: Gateway is running on 127.0.0.1:8080
```

## Making Your First Request

Send a chat completion request through the gateway:

```bash
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

## Health Check

Check the gateway health status:

```bash
curl http://localhost:8080/v1/health
```

Response:
```json
{
  "status": "healthy",
  "gateway": "yali",
  "version": "0.1.0",
  "stats": {
    "providers": 1,
    "backends": 1,
    "routes": 1
  }
}
```

## Next Steps

- [Configuration Guide](configuration.md) - Learn about all configuration options
- [Protocol Adapters](adapters.md) - Set up multi-provider support
- [Resilience Features](resilience.md) - Configure circuit breakers and retries
- [Load Balancing](load-balancing.md) - Configure traffic distribution

## Troubleshooting

### Gateway fails to start

1. Check if the port is already in use:
   ```bash
   lsof -i :8080
   ```

2. Verify the configuration file syntax:
   ```bash
   cat config.json | python -m json.tool
   ```

3. Check the log output for specific errors

### Requests return 404

Ensure your request path matches a configured route. The gateway uses prefix matching by default, so `/v1/chat` will match `/v1/chat/completions`.

### Authentication errors

Verify your API key is set correctly:
```bash
echo $OPENAI_API_KEY
```

For more help, check the [GitHub Issues](https://github.com/Tharsanan1/yali/issues) page.
