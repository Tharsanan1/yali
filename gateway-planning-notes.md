# Gateway Planning Notes

Date: 2026-02-07
Status: Collecting requirements

## Objectives
- (to be filled)

## Q&A Notes
- Q: Priority: MVP speed or long-term extensibility?
  A: Long-term extensibility.
- Q: Policy execution preference?
  A: Wasm (expects stable ABI so policy authors just follow spec).
- Q: Routing basis?
  A: Host, path, headers, method, query, body (all).
- Open: Control plane interface choice (REST vs gRPC) and whether to split user/admin API vs CP->DP sync channel.
- Q: Control plane interfaces?
  A: User/Admin -> Control Plane: REST. Control Plane -> Data Plane: gRPC streaming.
- Q: Policy ABI request/response access?
  A: Full access.
- Q: Body handling?
  A: Support streaming and buffered access with size limits.
- Q: Policy stage types?
  A: Pre-route + Pre-upstream + Post-response.
- Q: Policy capabilities (draft)?
  A: Policies can mutate headers/body on request and response, rewrite method/path, and influence upstream selection.
- Q: Upstream selection model?
  A: Policy returns a routing hint (router resolves to upstream).
- Open: Pre-route body access policy (allowed with limits vs restricted to preserve streaming).
- Q: Policy short-circuit response?
  A: Yes, policies can return a response with status/body.
- Q: Short-circuit allowed at which stages?
  A: All stages (Pre-Route, Pre-Upstream, Post-Response).
- Q: Pre-Route body access?
  A: No; body access only in Pre-Upstream (and Post-Response for response body).
- Q: Policy config format?
  A: JSON.
- Q: Policy ABI model choice?
  A: Wasm Component Model + WIT.
- Q: Failure mode on policy error?
  A: Fail-closed (block request on policy error).
- Open: Policy-to-policy data passing design (shared metadata bag, namespacing, limits).

## Decisions
- Config resolution (bootstrap): use Figment providers (file + env) for typed config extraction.
- Logging stack (recommended): tracing + tracing-subscriber JSON output; non-blocking writer via tracing-appender; bridge log crate via tracing-log; stdout by default; rolling file appender only when explicitly configured.
- NFR defaults (industry-standard starting points):
  - Added latency target: p95 <= 5-10ms (gateway overhead)
  - Throughput target: 10k rps per instance baseline (scale horizontally)
  - Availability target: 99.9%
  - Max request body size: 10 MB default (configurable; per-route overrides)
  - TLS termination: at gateway by default; passthrough optional per listener
  - Multi-tenant isolation: logical isolation via routing/headers; no hard process isolation by default

## Open Questions
- (to be filled)
  - Confirm logging outputs (stdout only vs stdout+rolling file) and OTLP export (later).

## Plan (Draft)
Goal: Build a high-performance, extensible Rust gateway on Pingora with dynamic Wasm policies (Component Model + WIT), REST control plane, and gRPC config streaming to data plane. Favor stable ABI and atomic config updates to keep latency low.

### Milestone 0: Repo + Baselines
- Add docs/structure for gateway components (data plane, control plane, policy ABI).
- Establish feature flags and crate layout (gateway-dp, gateway-cp, policy-sdk, policy-runtime).
- Define baseline performance targets and limits (see NFR defaults).
Deliverables:
- Architecture README and module map.
- Initial config schema for bootstrap (Figment).

### Milestone 1: Data Plane Core (Pingora)
- Implement Pingora listener, connection management, and upstream proxying.
- Add router (host/path/headers/method/query/body support).
- Implement routing hints and route resolution.
Performance notes:
- Keep routing on fast path; avoid body access unless explicitly requested by policy.
Deliverables:
- Basic L7 proxy with routing rules from in-memory config snapshot.

### Milestone 2: Policy Pipeline (Host + ABI)
- Implement 3-stage pipeline (Pre-Route, Pre-Upstream, Post-Response).
- Create versioned context objects per stage with a stable metadata bag.
- Define WIT interfaces for request/response/routing/context and decision output.
- Enforce fail-closed on policy errors.
- Enable short-circuit responses at all stages.
Design notes:
- Pre-Route body access: disabled.
- Pre-Upstream body access: allowed (streaming/buffered with limits).
- Post-Response body access: allowed.
- Body-based routing: supported via routing hints in Pre-Upstream with a single re-resolve pass (opt-in to avoid extra overhead).
Deliverables:
- `policy-sdk` with WIT and sample policy.
- `policy-runtime` host integration with Wasmtime.

### Milestone 3: Control Plane (REST) + Config Sync (gRPC)
- REST API for routes, policies, and policy chains.
- gRPC streaming to push config snapshots to data plane.
- Atomic config swap in data plane (versioned snapshots).
Deliverables:
- Control plane service with CRUD for routes/policies.
- DP subscriber that applies config updates safely.

### Milestone 4: Config Resolution
- Bootstrap config via Figment (file + env).
- Runtime config from CP: route graph + policy chains + limits.
Deliverables:
- Config schema and validation layer.
- Clear precedence rules (bootstrap < dynamic).

### Milestone 5: Observability + Safety
- Structured JSON logs via tracing/tracing-subscriber.
- Non-blocking logging via tracing-appender; optional rolling file appender only when configured.
- Metrics hooks and tracing spans across stages (request id, route id, policy id).
Deliverables:
- Logging + metrics wiring, baseline dashboards format (if/when OTLP chosen).

### Milestone 6: Testing + Perf
- Policy ABI conformance tests.
- Integration tests for policy pipeline stages and short-circuit behavior.
- Load tests against routing and policy overhead.
Deliverables:
- Test suite and perf baseline report.

### Milestone 7: Hardening + Extensibility
- ABI versioning strategy and compatibility tests.
- Policy marketplace packaging format (optional).
- Backward-compatible extension points for future capabilities.

## Milestone Backlog (Concrete)
Owners are placeholders; replace with actual names/roles.

### M0: Repo + Baselines
Owner: Platform Lead
Deliverables:
- Repo layout with crates: `gateway-dp`, `gateway-cp`, `policy-sdk`, `policy-runtime`.
- Architecture README (components + data/control plane flow).
- Bootstrap config schema and defaults (Figment).
Acceptance criteria:
- Builds succeed for all crates.
- README documents stage pipeline and config layering.
- Config parses from file + env with validation errors on invalid values.

### M1: Data Plane Core (Pingora)
Owner: Data Plane Lead
Deliverables:
- Pingora listener + TLS termination (configurable).
- Routing engine supporting host/path/headers/method/query/body.
- Routing hints support and route resolution.
- In-memory config snapshot for routes.
Acceptance criteria:
- Routes correctly selected for each routing basis.
- Default route used when no rules match.
- p95 added latency <= 10ms under baseline load (10k rps).

### M2: Policy Pipeline + ABI v1
Owner: Policy Platform Lead
Deliverables:
- 3-stage pipeline integrated in DP (Pre-Route, Pre-Upstream, Post-Response).
- WIT ABI v1 + generated bindings for at least Rust policy.
- Wasm runtime integration (Wasmtime).
- Fail-closed policy error handling.
- Short-circuit responses at all stages.
Acceptance criteria:
- Sample policy can mutate headers/method/path and set routing hint.
- Pre-Route cannot access body; Pre-Upstream can access body within limit.
- Policy trap returns configured error (fail-closed).

### M3: Control Plane + Config Sync
Owner: Control Plane Lead
Deliverables:
- REST Admin API for routes, policies, and policy chains.
- gRPC streaming channel for CP -> DP config snapshots.
- Versioned config snapshots with atomic swap.
Acceptance criteria:
- CRUD operations validate schemas and return versioned config.
- DP applies config updates without restart.
- No request sees partial config (atomic swap).

### M4: Config Resolution + Validation
Owner: Platform Lead
Deliverables:
- Figment provider stack (file + env) for bootstrap config.
- Validation layer for bootstrap and dynamic config.
- Precedence rules documented and enforced.
Acceptance criteria:
- Invalid configs fail fast with actionable errors.
- Dynamic config overrides bootstrap where appropriate.

### M5: Observability + Logging
Owner: Observability Lead
Deliverables:
- `tracing` instrumentation for request lifecycle + policy stages.
- JSON logs to stdout by default.
- Optional rolling file appender when configured.
- Correlation IDs (trace/request id) present in logs.
Acceptance criteria:
- Logs include route id, policy id, and stage.
- Non-blocking logging verified under load.

### M6: Testing + Performance
Owner: QA/Perf Lead
Deliverables:
- ABI conformance tests for policy runtime.
- Integration tests for routing + policy stages.
- Load test harness + baseline perf report.
Acceptance criteria:
- All tests pass in CI.
- Perf report meets NFR targets or documents gaps.

### M7: Hardening + Extensibility
Owner: Platform Lead
Deliverables:
- ABI versioning strategy and compatibility tests.
- Extension points and guidelines for future policy capabilities.
- Security review checklist for policies and CP->DP channel.
Acceptance criteria:
- ABI v1 -> v1.x additive changes validated.
- Documented process for extending context fields safely.


## Policy Stages (Locked, Extensible)
Note: Each stage must expose a versioned, extensible context object so we can add fields later without breaking the ABI.

### Pre-Route (before routing decision)
Visible to policy:
- Client metadata (IP, TLS info, protocol, connection id)
- Request line (method, path, query)
- Request headers
- Body access: TBD (opt-in with limits vs not available)
- Gateway context (time, trace id, tenant, request id)
Policy can:
- Allow/deny early
- Rewrite method/path/query
- Add/remove/normalize headers
- Set routing hints (route id, tenant, region, tags)
- Attach metadata for later stages

### Pre-Upstream (after route selection, before proxy)
Visible to policy:
- Everything from Pre-Route
- Selected route metadata
- Upstream selection (resolved target or pool)
Policy can:
- Enforce per-route policy
- Inject upstream auth headers
- Rewrite host/authority/path as needed
- Set upstream options (timeouts, retries, circuit-breaker hints)
- Modify request body (streaming or buffered, with limits)

### Post-Response (after upstream responds, before client write)
Visible to policy:
- Response status, headers, body stream
- Request metadata and routing context
- Upstream info (which target served it)
Policy can:
- Transform response headers/body
- Redact fields
- Add/override cache headers
- Metrics/logging/trace annotations
- Optionally replace/short-circuit response (if allowed by policy model)

## Policy Attachment Model (Route-Level)
Approach (preferred by user):
- Define policies as reusable units (ID + version + wasm blob ref).
- Attach policies directly on each route in explicit order.
- Each policy entry includes stage to preserve stage semantics.
Rules:
- Route-level policy list is authoritative.
- Optional global defaults can be added later as explicit prepend/append lists.
- Stage order is deterministic and documented.

## Upstream Model (Route-Level List)
Approach (preferred by user):
- Each route includes an ordered list of upstream targets.
- Load-balancing strategy is defined per route (e.g., round_robin, least_conn, random, hash).
- Each upstream entry can include weight, TLS override, metadata, and per-upstream health/outlier settings.
- Failover behavior (when to move to next priority/target) is defined at the route level.
