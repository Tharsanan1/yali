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
