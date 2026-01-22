# Yali AI Gateway Documentation

Welcome to the Yali AI Gateway documentation! This documentation covers how to get started, configure, and use the gateway effectively.

## Table of Contents

1. [Quickstart Guide](quickstart.md) - Get up and running in minutes
2. [Configuration Guide](configuration.md) - Detailed configuration options
3. [Protocol Adapters](adapters.md) - Multi-provider support and format translation
4. [Resilience Features](resilience.md) - Circuit breakers, retries, and health checks
5. [Load Balancing](load-balancing.md) - Traffic distribution algorithms

## Overview

Yali is a high-performance API Gateway specifically optimized for AI workloads. Built on Cloudflare's [Pingora](https://github.com/cloudflare/pingora) framework, it provides:

- **Protocol Adaptation**: Seamlessly translate between different AI provider formats (OpenAI, Azure, Anthropic, Google AI, AWS Bedrock)
- **Resilience**: Circuit breakers, retry with exponential backoff, and health checks
- **Load Balancing**: Failover, round-robin, weighted, and least-connections algorithms
- **Multi-tenancy**: Subdomain-based tenant isolation

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

## Quick Links

- [GitHub Repository](https://github.com/Tharsanan1/yali)
- [Report Issues](https://github.com/Tharsanan1/yali/issues)
- [Development Roadmap](../TODO.md)
