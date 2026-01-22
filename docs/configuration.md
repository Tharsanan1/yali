# Configuration Guide

Complete reference for configuring the Yali AI Gateway.

## Configuration File Structure

The gateway configuration is a JSON file with three main sections:

```json
{
  "providers": [...],  // AI provider endpoints
  "backends": [...],   // Routing and resilience policies
  "routes": [...]      // Traffic entry points
}
```

## Providers

A **Provider** represents a single AI endpoint.

### Basic Provider Configuration

```json
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
```

### Provider Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique identifier for the provider |
| `name` | No | Human-readable name |
| `spec.type` | Yes | Provider type (see [Adapters](adapters.md)) |
| `spec.endpoint` | Yes | Base URL for the provider |
| `spec.adapter` | No | Protocol adapter configuration |
| `spec.connection_pool` | No | Connection pool settings |

### Provider Types

| Type | Description |
|------|-------------|
| `openai` | OpenAI API |
| `azure_openai` | Azure OpenAI Service |
| `anthropic` | Anthropic Claude API |
| `google_ai` | Google AI (Gemini) |
| `bedrock` | AWS Bedrock |
| `custom` | Custom endpoint (uses OpenAI format) |

### Adapter Configuration

```json
{
  "adapter": {
    "auth": {
      "type": "bearer",
      "secret_ref": "env://API_KEY"
    },
    "url": {
      "path_prefix": "/v1/chat/completions",
      "path_template": "/openai/deployments/{model}/chat/completions",
      "query_params": {"api-version": "2024-02-15"}
    },
    "headers": {
      "add": {"X-Custom-Header": "value"},
      "remove": ["X-Unwanted-Header"]
    },
    "request_body": {
      "defaults": {"temperature": 0.7, "max_tokens": 1024},
      "remove_fields": ["user", "stream"]
    }
  }
}
```

### Authentication Types

| Type | Description | Example |
|------|-------------|---------|
| `bearer` | Authorization: ****** | `{"type": "bearer", "secret_ref": "env://KEY"}` |
| `header` | Custom header | `{"type": "header", "key": "x-api-key", "secret_ref": "..."}` |
| `query_param` | URL query parameter | `{"type": "query_param", "key": "key", "secret_ref": "..."}` |
| `none` | No authentication | `{"type": "none"}` |

### Secret References

| Format | Description |
|--------|-------------|
| `env://VAR_NAME` | Read from environment variable |

### Connection Pool Configuration

```json
{
  "connection_pool": {
    "max_connections": 100,
    "idle_timeout": "60s",
    "max_idle_per_host": 10
  }
}
```

## Backends

A **Backend** groups providers and applies policies.

### Basic Backend Configuration

```json
{
  "id": "backend_main",
  "name": "Main Backend",
  "spec": {
    "load_balancing": {
      "algorithm": "failover"
    },
    "providers": [
      {"ref": "provider_openai", "priority": 1, "weight": 100},
      {"ref": "provider_azure", "priority": 2, "weight": 100}
    ]
  }
}
```

### Backend Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique identifier |
| `name` | No | Human-readable name |
| `spec.load_balancing` | No | Load balancing configuration |
| `spec.providers` | Yes | List of provider references |
| `spec.timeout` | No | Timeout settings |
| `spec.retry` | No | Retry policy |
| `spec.circuit_breaker` | No | Circuit breaker settings |
| `spec.health_check` | No | Health check configuration |

### Load Balancing Configuration

```json
{
  "load_balancing": {
    "algorithm": "failover"  // failover, round_robin, weighted, least_connections
  }
}
```

See [Load Balancing](load-balancing.md) for details.

### Provider References

```json
{
  "providers": [
    {
      "ref": "provider_id",  // References a provider by ID
      "priority": 1,          // Lower = higher priority (for failover)
      "weight": 100           // Weight for weighted load balancing
    }
  ]
}
```

### Timeout Configuration

```json
{
  "timeout": {
    "connect": "2s",     // TCP connection timeout
    "response": "600s",  // Time to wait for response
    "idle": "60s"        // Connection idle timeout
  }
}
```

### Retry Configuration

```json
{
  "retry": {
    "attempts": 3,
    "backoff": {
      "initial": "100ms",
      "max": "10s",
      "multiplier": 2
    },
    "conditions": ["5xx", "connect-failure", "reset", "timeout"]
  }
}
```

### Circuit Breaker Configuration

```json
{
  "circuit_breaker": {
    "enabled": true,
    "error_threshold_percentage": 50,
    "min_request_volume": 20,
    "sleep_window": "30s",
    "half_open_requests": 5
  }
}
```

See [Resilience Features](resilience.md) for details.

### Health Check Configuration

```json
{
  "health_check": {
    "type": "passive",           // passive or active
    "interval": "10s",           // Check interval (active only)
    "timeout": "5s",             // Check timeout
    "path": "/health",           // Health check path (active only)
    "healthy_threshold": 2,      // Successes to mark healthy
    "unhealthy_threshold": 3,    // Failures to mark unhealthy
    "expected_statuses": [200, 204]
  }
}
```

## Routes

A **Route** defines how traffic enters the gateway.

### Basic Route Configuration

```json
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
```

### Route Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique identifier |
| `host` | No | Host/domain for multi-tenant routing |
| `spec.match` | Yes | Match criteria |
| `spec.backend_ref` | Yes | Backend to route to |
| `spec.filters` | No | Request/response filters |

### Host-Based Routing (Multi-Tenant)

```json
{
  "id": "route_tenant_a",
  "host": "tenant-a.gateway.example.com",
  "spec": {
    "match": {"path": "/v1/chat", "type": "prefix"},
    "backend_ref": "backend_tenant_a"
  }
}
```

### Path Matching

| Type | Description | Example |
|------|-------------|---------|
| `prefix` | Matches path prefix | `/v1/chat` matches `/v1/chat/completions` |
| `exact` | Matches exact path | `/health` matches only `/health` |

### Method Filtering

```json
{
  "match": {
    "path": "/v1/chat",
    "methods": ["POST"]  // Only POST requests
  }
}
```

## Complete Configuration Example

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
          "auth": {"type": "bearer", "secret_ref": "env://OPENAI_API_KEY"},
          "url": {"path_prefix": "/v1/chat/completions"}
        }
      }
    },
    {
      "id": "provider_azure",
      "name": "Azure OpenAI Backup",
      "spec": {
        "type": "azure_openai",
        "endpoint": "https://myresource.openai.azure.com",
        "adapter": {
          "auth": {"type": "header", "key": "api-key", "secret_ref": "env://AZURE_KEY"},
          "url": {"query_params": {"api-version": "2024-02-15-preview"}}
        }
      }
    }
  ],
  "backends": [
    {
      "id": "backend_main",
      "name": "Main Backend",
      "spec": {
        "load_balancing": {"algorithm": "failover"},
        "providers": [
          {"ref": "provider_openai", "priority": 1, "weight": 100},
          {"ref": "provider_azure", "priority": 2, "weight": 100}
        ],
        "timeout": {
          "connect": "5s",
          "response": "300s"
        },
        "retry": {
          "attempts": 3,
          "backoff": {"initial": "100ms", "max": "5s", "multiplier": 2},
          "conditions": ["5xx", "connect-failure"]
        },
        "health_check": {
          "type": "passive",
          "unhealthy_threshold": 3,
          "healthy_threshold": 2
        }
      }
    }
  ],
  "routes": [
    {
      "id": "route_chat",
      "spec": {
        "match": {"path": "/v1/chat", "type": "prefix", "methods": ["POST"]},
        "backend_ref": "backend_main"
      }
    },
    {
      "id": "route_embeddings",
      "spec": {
        "match": {"path": "/v1/embeddings", "type": "prefix", "methods": ["POST"]},
        "backend_ref": "backend_main"
      }
    }
  ]
}
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GATEWAY_LISTEN_ADDR` | Address to listen on | `0.0.0.0:8080` |
| `RUST_LOG` | Log level | `info` |

## Validation

The gateway validates configuration on startup:

1. All provider references in backends must exist
2. All backend references in routes must exist
3. Provider IDs must be unique
4. Backend IDs must be unique
5. Route IDs must be unique

Invalid configurations will cause the gateway to exit with an error message.
