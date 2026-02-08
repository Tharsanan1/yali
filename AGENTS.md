# AGENTS Guide for Yali

This file is for coding agents working in `/Users/tharsanan/Documents/Projects/yali`.

## Project Overview

Yali is a Rust API gateway with a control-plane/data-plane split.

- `gateway-cp`: REST admin API + gRPC snapshot publisher.
- `gateway-dp`: Pingora proxy, route matching, policy execution.
- `gateway-proto`: protobuf contract between CP and DP.
- `policy-runtime`: Wasmtime component host runtime.
- `policy-sdk`: WIT contract used by policies.
- `policies/*`: policy component crates/artifacts.
- `gateway-it`: cucumber integration tests.

## Architectural Rules

- Keep CP and DP decoupled; the protobuf snapshot is the contract.
- Treat policy behavior as generic action execution, never hardcoded policy IDs.
- Route-attached policies execute in declared order.
- Fail closed on policy preload/execution errors (return `500` in DP).
- Use atomic snapshot swap in DP; keep previous active snapshot on apply failure.

## Rust Structure Best Practices

- Keep handlers thin in CP:
  - request parsing/validation at the edge
  - business logic in service modules
  - persistence in repository/store modules
- Keep DP hot path minimal:
  - avoid per-request expensive allocations when possible
  - avoid blocking calls in request path
- Keep cross-crate types explicit and versioned through `gateway-proto`.

## Run Commands

### Build

```bash
cargo check --workspace
cargo build -p gateway-cp -p gateway-dp -p gateway-it
```

### Local process integration

```bash
make it-local-build
make it-local-up
make it-local-test
make it-local-down
```

One-shot:

```bash
make it-local
```

### Docker integration

```bash
docker compose -f docker-compose.yml -f docker-compose.it.yml build
docker compose -f docker-compose.yml -f docker-compose.it.yml run --rm gateway-it
```

### Runtime stack only (no test services)

```bash
docker compose up -d gateway-cp gateway-dp
```

## Policy Development Workflow

1. Implement policy in `policies/<name>/src`.
2. Export the WIT world defined in `policy-sdk/wit/policy.wit`.
3. Build artifacts:
   - `./scripts/build-policy-artifacts.sh`
4. Register policy via CP `POST /policies` with:
   - `id`, `version`, `wasm_uri`, `sha256`
   - `supported_stages`, `config_schema`, `default_config`
5. Attach policy to route via CP `POST /routes` using `params`.
6. Verify with cucumber features in `gateway-it/features`.

## Testing Rules

- Prefer cucumber for integration behavior.
- Add/extend generic steps instead of creating highly specific one-off steps.
- When behavior is policy-specific, still validate through generic HTTP/JSON steps.
- Keep existing scenarios passing when adding new features.

## Logging and Debugging

- Process IT logs:
  - `/Users/tharsanan/Documents/Projects/yali/target/it-local/logs/gateway-cp.log`
  - `/Users/tharsanan/Documents/Projects/yali/target/it-local/logs/gateway-dp.log`
  - `/Users/tharsanan/Documents/Projects/yali/target/it-local/logs/upstream.log`
- Common failure classes:
  - protobuf mismatch between CP/DP
  - policy artifact SHA mismatch
  - unsupported decision action for the current stage

## Change Management

- If `gateway-proto/proto/gateway.proto` changes:
  - regenerate code and update CP + DP in same branch/PR.
- Do not introduce breaking API/wire changes in one side only.
- Keep docs updated when behavior changes:
  - `README.md`
  - `docs/policy-authoring.md`
  - `docs/architecture.md` if flow changes

## Guardrails

- Do not hardcode policy IDs in DP/CP behavior.
- Do not silently ignore policy errors.
- Do not modify unrelated files in the same commit.
- Keep commits scoped and verifiable with at least one test command run.
