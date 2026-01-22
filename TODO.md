# Yali AI-Native Gateway - Development Roadmap

This document tracks the implementation progress of the AI-Native Gateway based on the [PRD specification](./prd).

## Current Status

**Phase 1: Core Foundation** - Partially Complete ✅

---

## Phase 1: Core Foundation (Weeks 1-3)

**Goal:** Basic proxy functionality with config management

- [x] Implement `Provider`, `Backend`, and `Route` Rust structs with validation
- [x] Build `RadixRouter` lookup logic in Pingora's `upstream_peer`
- [x] Implement pass-through proxying (no filters)
- [x] Implement local config file loading
- [ ] Add basic health endpoint (`/v1/health`)
- [ ] Set up Prometheus metrics endpoint
- [x] **Deliverable:** Gateway that routes requests based on static config file

### Implementation Notes (Phase 1)
- Provider, Backend, Route structs implemented in `src/config/mod.rs`
- RadixRouter using `matchit` crate for longest-prefix-match in `src/state.rs`
- Pingora `ProxyHttp` trait implemented in `src/proxy.rs`
- JSON config file loading with referential integrity validation

---

## Phase 2: Resilience & Connectivity (Weeks 4-6)

**Goal:** Production-grade connection handling

- [ ] Implement `ProtocolAdapter` trait and body transformation engine
- [ ] Build adapters for OpenAI, Azure, Anthropic, Google AI, Bedrock
- [ ] Implement connection pool sharing per Provider
- [ ] Add circuit breaker per Provider
- [ ] Implement active health checks
- [ ] Add retry with exponential backoff
- [ ] Support multiple load balancing algorithms (currently only failover)
- [ ] **Deliverable:** Gateway with failover, health checks, and protocol adaptation

### Partial Progress
- Basic URL rewriting and auth header injection implemented
- Failover load balancing algorithm implemented
- Config structures support circuit breaker, retry, health check settings (not yet enforced)

---

## Phase 3: Filter Framework (Weeks 7-9)

**Goal:** Extensible filter pipeline with core filters

- [ ] Implement `Filter` trait with phases (PRE_ROUTE, PRE_BACKEND, etc.)
- [ ] Build `FilterRegistry` for dynamic filter loading
- [ ] Implement `FilterContext` for shared state between filters
- [ ] Build `FilterPipeline` executor with phase ordering
- [ ] Implement `auth_jwt` filter
- [ ] Implement `auth_api_key` filter with Redis backend
- [ ] Implement basic `transform_headers` filter
- [ ] Implement basic `transform_body` filter
- [ ] Support SSE streaming passthrough
- [ ] **Deliverable:** Gateway with authentication and basic transformations

---

## Phase 4: Distributed State & Rate Limiting (Weeks 10-12)

**Goal:** Production-grade rate limiting and caching

- [ ] Implement `StateStore` trait and Redis implementation
- [ ] Build `rate_limit` filter with multiple algorithms
- [ ] Support request-based and token-based rate limiting
- [ ] Implement `concurrent_limit` filter
- [ ] Implement `quota` filter for usage limits
- [ ] Implement `token_counter` filter (tiktoken integration)
- [ ] Add rate limit response headers
- [ ] **Deliverable:** Gateway with distributed rate limiting

---

## Phase 5: AI-Specific Filters (Weeks 13-15)

**Goal:** AI-native capabilities

- [ ] Implement `pii_masking` filter (streaming body inspection)
- [ ] Implement `prompt_injection` filter (pattern + classifier)
- [ ] Implement `content_filter` filter
- [ ] Implement `semantic_cache` filter with vector store
- [ ] Implement `cost_tracker` filter
- [ ] Implement `model_router` filter
- [ ] **Deliverable:** Gateway with AI-specific security and optimization

---

## Phase 6: Controller Integration (Weeks 16-18)

**Goal:** Dynamic configuration and multi-gateway management

- [ ] Define gRPC proto for controller ↔ gateway communication
- [ ] Implement config sync protocol (full + incremental)
- [ ] Add local snapshot persistence (for controller unavailability)
- [ ] Implement hot config reload (zero-downtime updates)
- [ ] Add config version validation
- [ ] Implement controller health reporting
- [ ] Build API key management endpoints
- [ ] Build usage statistics endpoints
- [ ] **Deliverable:** Gateway that syncs config from external controller

---

## Phase 7: Observability & Hardening (Weeks 19-20)

**Goal:** Production readiness

- [ ] Integrate OpenTelemetry tracing with filter spans
- [ ] Add structured JSON logging
- [ ] Implement audit logging for config changes
- [ ] Performance testing and optimization
- [ ] Security audit and penetration testing
- [ ] Documentation and runbooks
- [ ] **Deliverable:** Production-ready gateway with full observability

---

## API Endpoints Status

| Method | Endpoint | Status |
|--------|----------|--------|
| `POST` | `/v1/providers` | ❌ Not implemented (config file only) |
| `GET` | `/v1/providers` | ❌ Not implemented |
| `GET` | `/v1/providers/{id}` | ❌ Not implemented |
| `PUT` | `/v1/providers/{id}` | ❌ Not implemented |
| `PATCH` | `/v1/providers/{id}` | ❌ Not implemented |
| `DELETE` | `/v1/providers/{id}` | ❌ Not implemented |
| `POST` | `/v1/backends` | ❌ Not implemented |
| `GET` | `/v1/backends` | ❌ Not implemented |
| `GET` | `/v1/backends/{id}` | ❌ Not implemented |
| `PUT` | `/v1/backends/{id}` | ❌ Not implemented |
| `PATCH` | `/v1/backends/{id}` | ❌ Not implemented |
| `DELETE` | `/v1/backends/{id}` | ❌ Not implemented |
| `POST` | `/v1/routes` | ❌ Not implemented |
| `GET` | `/v1/routes` | ❌ Not implemented |
| `GET` | `/v1/routes/{id}` | ❌ Not implemented |
| `PUT` | `/v1/routes/{id}` | ❌ Not implemented |
| `PATCH` | `/v1/routes/{id}` | ❌ Not implemented |
| `DELETE` | `/v1/routes/{id}` | ❌ Not implemented |
| `GET` | `/v1/health` | ❌ Not implemented |
| `GET` | `/v1/metrics` | ❌ Not implemented |

---

## Provider Types Implementation Status

| Type | Status | Notes |
|------|--------|-------|
| `openai` | ✅ Basic | URL rewriting, bearer auth |
| `azure_openai` | ❌ | Not implemented |
| `anthropic` | ❌ | Not implemented |
| `google_ai` | ❌ | Not implemented |
| `bedrock` | ❌ | Not implemented |
| `custom` | ✅ Basic | Generic passthrough |

---

## Filter Implementation Status

### Authentication Filters
| Filter | Status |
|--------|--------|
| `auth_jwt` | ❌ Not implemented |
| `auth_api_key` | ❌ Not implemented |
| `auth_oauth2` | ❌ Not implemented |
| `auth_mtls` | ❌ Not implemented |
| `auth_basic` | ❌ Not implemented |

### Rate Limiting Filters
| Filter | Status |
|--------|--------|
| `rate_limit` | ❌ Not implemented |
| `concurrent_limit` | ❌ Not implemented |
| `quota` | ❌ Not implemented |

### Transformation Filters
| Filter | Status |
|--------|--------|
| `transform_headers` | ❌ Not implemented |
| `transform_body` | ❌ Not implemented |
| `transform_url` | ❌ Not implemented |

### AI-Specific Filters
| Filter | Status |
|--------|--------|
| `token_counter` | ❌ Not implemented |
| `pii_masking` | ❌ Not implemented |
| `prompt_injection` | ❌ Not implemented |
| `content_filter` | ❌ Not implemented |
| `semantic_cache` | ❌ Not implemented |
| `cost_tracker` | ❌ Not implemented |
| `model_router` | ❌ Not implemented |
| `fallback_response` | ❌ Not implemented |

### Observability Filters
| Filter | Status |
|--------|--------|
| `request_logger` | ❌ Not implemented |
| `metrics` | ❌ Not implemented |
| `trace_enricher` | ❌ Not implemented |

---

## Contributing

To work on the next tasks:

1. Pick an unchecked item from the current phase
2. Create a feature branch
3. Implement the feature with tests
4. Submit a PR referencing this TODO

See [README.md](./README.md) for development setup instructions.
