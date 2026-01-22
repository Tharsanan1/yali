# Load Balancing

Yali Gateway supports multiple load balancing algorithms to distribute traffic across multiple AI providers efficiently.

## Overview

Load balancing determines how requests are distributed across healthy providers within a backend. The gateway supports four algorithms:

| Algorithm | Behavior | Best For |
|-----------|----------|----------|
| `failover` | Uses highest priority provider; falls back on failure | Primary/backup setups |
| `round_robin` | Distributes evenly across healthy providers | Even load distribution |
| `weighted` | Distributes based on weight values | Gradual rollouts, A/B testing |
| `least_connections` | Routes to provider with fewest active connections | Variable latency providers |

## Configuration

Configure load balancing in the backend specification:

```json
{
  "backends": [{
    "id": "backend_main",
    "spec": {
      "load_balancing": {
        "algorithm": "failover"
      },
      "providers": [
        {"ref": "provider_primary", "priority": 1, "weight": 100},
        {"ref": "provider_backup", "priority": 2, "weight": 50}
      ]
    }
  }]
}
```

## Failover (Default)

The failover algorithm routes all traffic to the highest priority (lowest number) healthy provider. If that provider fails, traffic automatically shifts to the next priority provider.

### Configuration

```json
{
  "load_balancing": {"algorithm": "failover"},
  "providers": [
    {"ref": "provider_openai", "priority": 1},
    {"ref": "provider_azure", "priority": 2},
    {"ref": "provider_anthropic", "priority": 3}
  ]
}
```

### Behavior

```
┌────────────────────────────────────────────────────────────┐
│                     FAILOVER ROUTING                        │
├────────────────────────────────────────────────────────────┤
│                                                            │
│   Request ──►  OpenAI (priority 1)     [HEALTHY] ◄── Used  │
│                Azure (priority 2)      [HEALTHY]           │
│                Anthropic (priority 3)  [HEALTHY]           │
│                                                            │
│   If OpenAI fails:                                         │
│                                                            │
│   Request ──►  OpenAI (priority 1)     [UNHEALTHY]         │
│                Azure (priority 2)      [HEALTHY] ◄── Used  │
│                Anthropic (priority 3)  [HEALTHY]           │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Use Cases

- Primary/backup provider setups
- Cost optimization (use cheaper provider first)
- Vendor lock-in mitigation

## Round Robin

The round-robin algorithm distributes requests evenly across all healthy providers in a circular fashion.

### Configuration

```json
{
  "load_balancing": {"algorithm": "round_robin"},
  "providers": [
    {"ref": "provider_1", "priority": 1},
    {"ref": "provider_2", "priority": 1},
    {"ref": "provider_3", "priority": 1}
  ]
}
```

### Behavior

```
Request 1 ──► Provider 1
Request 2 ──► Provider 2
Request 3 ──► Provider 3
Request 4 ──► Provider 1  (cycles back)
Request 5 ──► Provider 2
...
```

### Use Cases

- Even load distribution
- Multiple equivalent providers
- Maximizing throughput across providers

## Weighted

The weighted algorithm distributes requests based on configured weight values. Providers with higher weights receive proportionally more traffic.

### Configuration

```json
{
  "load_balancing": {"algorithm": "weighted"},
  "providers": [
    {"ref": "provider_main", "priority": 1, "weight": 80},
    {"ref": "provider_new", "priority": 1, "weight": 20}
  ]
}
```

### Behavior

With the above configuration:
- `provider_main` receives ~80% of requests
- `provider_new` receives ~20% of requests

```
┌────────────────────────────────────────────────────────────┐
│                   WEIGHTED DISTRIBUTION                     │
├────────────────────────────────────────────────────────────┤
│                                                            │
│   100 requests distributed as:                             │
│                                                            │
│   provider_main (weight 80)  ████████████████████ ~80 reqs │
│   provider_new  (weight 20)  █████ ~20 reqs                │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Use Cases

- Gradual rollouts (shift traffic slowly to new provider)
- A/B testing between providers
- Capacity-based routing (route more to higher-capacity providers)

### Canary Deployment Example

Gradually shift traffic to a new provider:

```json
// Week 1: 5% to new provider
{"ref": "provider_stable", "weight": 95},
{"ref": "provider_new", "weight": 5}

// Week 2: 20% to new provider
{"ref": "provider_stable", "weight": 80},
{"ref": "provider_new", "weight": 20}

// Week 3: 50/50
{"ref": "provider_stable", "weight": 50},
{"ref": "provider_new", "weight": 50}

// Week 4: 100% to new provider
{"ref": "provider_new", "weight": 100}
```

## Least Connections

The least-connections algorithm routes requests to the provider with the fewest active connections. This is ideal when provider latencies vary significantly.

### Configuration

```json
{
  "load_balancing": {"algorithm": "least_connections"},
  "providers": [
    {"ref": "provider_fast", "priority": 1},
    {"ref": "provider_slow", "priority": 1}
  ]
}
```

### Behavior

```
┌────────────────────────────────────────────────────────────┐
│                 LEAST CONNECTIONS ROUTING                   │
├────────────────────────────────────────────────────────────┤
│                                                            │
│   provider_fast: 2 active connections                      │
│   provider_slow: 5 active connections                      │
│                                                            │
│   New request ──► provider_fast (fewer connections)        │
│                                                            │
│   After some time:                                         │
│                                                            │
│   provider_fast: 4 active connections                      │
│   provider_slow: 3 active connections                      │
│                                                            │
│   New request ──► provider_slow (fewer connections now)    │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Use Cases

- Providers with variable response times
- Handling long-running AI requests (streaming, large completions)
- Optimizing resource utilization

## Combining with Health Checks

Load balancing automatically considers provider health. Unhealthy providers are excluded from selection:

```json
{
  "load_balancing": {"algorithm": "round_robin"},
  "health_check": {
    "type": "passive",
    "unhealthy_threshold": 3
  },
  "providers": [
    {"ref": "provider_1"},
    {"ref": "provider_2"},
    {"ref": "provider_3"}
  ]
}
```

If `provider_2` becomes unhealthy after 3 consecutive failures:

```
Request 1 ──► Provider 1
Request 2 ──► Provider 3  (skips unhealthy provider_2)
Request 3 ──► Provider 1
...
```

## Multi-Region Load Balancing

Configure load balancing for geographic distribution:

```json
{
  "backends": [{
    "id": "backend_global",
    "spec": {
      "load_balancing": {"algorithm": "failover"},
      "providers": [
        {"ref": "provider_us_east", "priority": 1, "weight": 100},
        {"ref": "provider_us_west", "priority": 2, "weight": 100},
        {"ref": "provider_eu", "priority": 3, "weight": 100}
      ]
    }
  }]
}
```

## Best Practices

1. **Start with Failover**: Simple and reliable for most use cases
2. **Use Weighted for Migrations**: Gradually shift traffic to new providers
3. **Use Least Connections for Variable Latency**: When response times differ significantly
4. **Combine with Health Checks**: Always enable health checking for automatic failover
5. **Set Appropriate Priorities**: Lower numbers = higher priority in failover mode
6. **Monitor Connection Counts**: Track metrics to tune least_connections thresholds

## Metrics

The gateway exposes load balancing metrics:

| Metric | Description |
|--------|-------------|
| `provider_requests_total` | Total requests per provider |
| `provider_active_connections` | Current active connections per provider |
| `provider_errors_total` | Total errors per provider |
| `load_balancer_selections` | Selection counts per algorithm |
