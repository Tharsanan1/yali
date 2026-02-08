# Yali Gateway

A Rust gateway based on Pingora, designed for high performance and long-term extensibility with a Wasm policy pipeline.

**Architecture Overview**

See `docs/architecture.md` for the full diagram and data/control plane flow.

**Crates**
- `gateway-dp` Data plane. Pingora listener, routing, policy stages.
- `gateway-cp` Control plane. REST admin API and gRPC config streaming.
- `policy-sdk` WIT-based ABI bindings and helpers for policy authors.
- `policy-runtime` Wasmtime host integration and policy lifecycle.

**Bootstrap Config**
- Example data plane config: `config/gateway.example.toml`
- Example control plane config: `config/control-plane.example.toml`
- Config layering: file + env via Figment.

**Milestones**
- See `gateway-planning-notes.md` for the milestone backlog and decisions.

**Docker**
- Runtime services only:
  - `docker compose up -d gateway-cp gateway-dp`
- Integration tests (base + IT override):
  - `docker compose -f docker-compose.yml -f docker-compose.it.yml build`
  - `docker compose -f docker-compose.yml -f docker-compose.it.yml run --rm gateway-it`
- Note: test-only services (`upstream`, `gateway-it`) live in `docker-compose.it.yml`.

**Local Integration Tests (Process Mode)**
- Build binaries:
  - `make it-local-build`
- Start local upstream + CP + DP:
  - `make it-local-up`
- Run cucumber integration tests against local processes:
  - `make it-local-test`
- Stop local processes:
  - `make it-local-down`
- One-shot run (`up + test + down`):
  - `make it-local`
- Skip rebuild in `up`/`run` when binaries are already built:
  - `IT_LOCAL_SKIP_BUILD=1 make it-local`
