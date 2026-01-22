# Resilience Features

Yali Gateway provides robust resilience features to ensure your AI applications remain available even when individual providers experience issues.

## Overview

The gateway implements several resilience patterns:

- **Circuit Breaker**: Prevents cascading failures by stopping requests to failing providers
- **Retry with Exponential Backoff**: Automatically retries failed requests
- **Health Checks**: Monitors provider health to make intelligent routing decisions

## Circuit Breaker

The circuit breaker pattern prevents your system from repeatedly trying to execute operations that are likely to fail, giving the failing service time to recover.

### How It Works

```
┌────────────────────────────────────────────────────────────┐
│                  CIRCUIT BREAKER STATES                     │
├────────────────────────────────────────────────────────────┤
│                                                            │
│   ┌──────────┐  error rate ≥ threshold  ┌──────────┐      │
│   │  CLOSED  │ ─────────────────────────►│   OPEN   │      │
│   │ (Normal) │                           │(Blocking)│      │
│   └────▲─────┘                           └────┬─────┘      │
│        │                                      │            │
│        │              sleep window expires    │            │
│        │                                      ▼            │
│        │  success    ┌───────────┐                         │
│        └─────────────│ HALF-OPEN │                         │
│                      │ (Testing) │                         │
│        ┌─────────────│           │◄────────────────────────┘
│        │   failure   └───────────┘                         │
│        │                                                   │
│        ▼                                                   │
│   Back to OPEN                                             │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### States

| State | Description | Behavior |
|-------|-------------|----------|
| **Closed** | Normal operation | Requests flow through normally |
| **Open** | Circuit tripped | All requests are immediately rejected |
| **Half-Open** | Testing recovery | Limited requests allowed to test if provider recovered |

### Configuration

Configure circuit breaker per backend:

```json
{
  "backends": [{
    "id": "backend_main",
    "spec": {
      "circuit_breaker": {
        "enabled": true,
        "error_threshold_percentage": 50,
        "min_request_volume": 20,
        "sleep_window": "30s",
        "half_open_requests": 5
      },
      "providers": [...]
    }
  }]
}
```

### Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `enabled` | Enable/disable circuit breaker | `false` |
| `error_threshold_percentage` | Error rate to trip circuit (0-100) | `50` |
| `min_request_volume` | Minimum requests before circuit can trip | `20` |
| `sleep_window` | Time to wait before transitioning to half-open | `30s` |
| `half_open_requests` | Number of test requests in half-open state | `5` |

### Example Scenario

1. Provider starts having issues (50%+ errors)
2. After 20 requests with ≥50% error rate, circuit opens
3. All subsequent requests fail fast (no upstream call)
4. After 30 seconds, circuit moves to half-open
5. 5 test requests are allowed through
6. If all 5 succeed, circuit closes
7. If any fail, circuit reopens for another 30 seconds

## Retry with Exponential Backoff

Automatically retry failed requests with increasing delays between attempts.

### Configuration

```json
{
  "backends": [{
    "id": "backend_main",
    "spec": {
      "retry": {
        "attempts": 3,
        "backoff": {
          "initial": "100ms",
          "max": "10s",
          "multiplier": 2
        },
        "conditions": ["5xx", "connect-failure", "reset"]
      },
      "providers": [...]
    }
  }]
}
```

### Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `attempts` | Maximum number of retry attempts | `3` |
| `backoff.initial` | Initial backoff delay | `100ms` |
| `backoff.max` | Maximum backoff delay | `10s` |
| `backoff.multiplier` | Backoff multiplier | `2.0` |
| `conditions` | Conditions that trigger a retry | `["5xx", "connect-failure", "reset"]` |

### Retry Conditions

| Condition | Description |
|-----------|-------------|
| `5xx` | Retry on server errors (500-599) |
| `connect-failure` | Retry on connection failures |
| `reset` | Retry on connection resets |
| `timeout` | Retry on request timeouts |
| `429` | Retry on rate limit responses |

### Backoff Example

With default settings:
- Attempt 1: Immediate
- Attempt 2 (on failure): Wait 100ms
- Attempt 3 (on failure): Wait 200ms
- Attempt 4 (on failure): Wait 400ms

### Best Practices

1. **Don't retry on 4xx errors**: Client errors won't succeed on retry
2. **Set reasonable max attempts**: 3-5 is usually sufficient
3. **Respect rate limits**: Add `429` to conditions and increase backoff
4. **Consider idempotency**: Only retry operations that are safe to repeat

## Health Checks

Monitor provider health to make intelligent routing decisions.

### Types of Health Checks

| Type | Description | Use Case |
|------|-------------|----------|
| **Passive** | Infer health from regular traffic | Default, zero overhead |
| **Active** | Periodically send health check requests | When traffic is low |

### Configuration

```json
{
  "backends": [{
    "id": "backend_main",
    "spec": {
      "health_check": {
        "type": "passive",
        "healthy_threshold": 2,
        "unhealthy_threshold": 3,
        "expected_statuses": [200, 204]
      },
      "providers": [...]
    }
  }]
}
```

### Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `type` | `passive` or `active` | `passive` |
| `interval` | Check interval (active only) | `10s` |
| `timeout` | Check timeout | `5s` |
| `path` | Health check path (active only) | None |
| `healthy_threshold` | Consecutive successes to mark healthy | `2` |
| `unhealthy_threshold` | Consecutive failures to mark unhealthy | `3` |
| `expected_statuses` | HTTP statuses considered healthy | `[200, 204]` |

### Passive Health Checks

Passive health checking monitors regular traffic without sending additional requests:

```
Provider Response → Gateway Records Result → Health State Updated
```

- **Success**: Increment consecutive successes counter
- **5xx Error**: Increment consecutive failures counter
- **4xx Error**: Ignored (client error, not provider issue)

### Health States

| State | Description | Behavior |
|-------|-------------|----------|
| **Unknown** | Not yet determined | Treated as healthy |
| **Healthy** | Provider is working | Normal routing |
| **Unhealthy** | Provider is failing | Excluded from load balancing |

## Timeouts

Configure timeouts for each phase of request processing:

```json
{
  "backends": [{
    "id": "backend_main",
    "spec": {
      "timeout": {
        "connect": "2s",
        "response": "600s",
        "idle": "60s"
      },
      "providers": [...]
    }
  }]
}
```

### Timeout Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `connect` | TCP connection timeout | `2s` |
| `response` | Timeout waiting for response | `600s` |
| `idle` | Connection idle timeout | `60s` |

### AI Workload Considerations

AI requests can take much longer than typical HTTP requests:
- GPT-4 with long context: 30-60 seconds
- Long completions: 60-120 seconds
- Streaming requests: May need even longer

Set `response` timeout accordingly (default 600s = 10 minutes).

## Combining Resilience Features

All resilience features work together:

```json
{
  "backends": [{
    "id": "backend_resilient",
    "spec": {
      "load_balancing": {
        "algorithm": "failover"
      },
      "providers": [
        {"ref": "provider_primary", "priority": 1},
        {"ref": "provider_backup", "priority": 2}
      ],
      "timeout": {
        "connect": "2s",
        "response": "300s"
      },
      "retry": {
        "attempts": 3,
        "backoff": {"initial": "100ms", "max": "5s", "multiplier": 2},
        "conditions": ["5xx", "connect-failure"]
      },
      "circuit_breaker": {
        "enabled": true,
        "error_threshold_percentage": 50,
        "min_request_volume": 10,
        "sleep_window": "30s"
      },
      "health_check": {
        "type": "passive",
        "unhealthy_threshold": 3,
        "healthy_threshold": 2
      }
    }
  }]
}
```

### Request Flow with Resilience

1. **Request arrives** at route
2. **Circuit breaker check**: Is circuit open? If yes, fail fast
3. **Provider selection**: Choose healthy provider based on load balancing
4. **Health check**: Is selected provider healthy? If not, try next
5. **Attempt request** with configured timeout
6. **On success**: Record success for health check and circuit breaker
7. **On failure**:
   - Record failure for health check and circuit breaker
   - Check retry conditions
   - If should retry, wait for backoff delay
   - Go to step 3 (select next provider if failover)
8. **Return response** or final error to client
