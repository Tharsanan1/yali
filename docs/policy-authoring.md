# Custom Policy Authoring and Testing

This guide explains how to add a custom policy in the current gateway codebase and validate it end to end.

## Current Runtime Model

Today, the control plane and data plane behave as follows:

- Control plane (`gateway-cp`) validates policy configs with JSON Schema.
- Routes attach policies with per-route `params`.
- Effective config is `deep_merge(default_config, params)` and is sent to DP over gRPC.
- Data plane (`gateway-dp`) loads policy artifacts (`wasm_uri`, `sha256`) and validates module bytes.
- Data plane executes attached policies in route order and parses generic policy actions from effective config (`request_headers`, `request_rewrite`, `upstream_hint`, `direct_response`, `request_body_patch`, `response_headers`).

Important: the WIT contract exists at `policy-sdk/wit/policy.wit`, but runtime is not yet invoking guest policy code through Component Model bindings. Current execution is host-side action interpretation with fail-closed behavior.

## 1) Define the Policy Contract

Pick a unique `(id, version)` and define:

- `supported_stages` (currently only `pre_upstream` is executed in DP).
- `config_schema` (JSON Schema).
- `default_config` (must satisfy schema).

Example registration payload:

```json
{
  "id": "my-policy",
  "version": "1.0.0",
  "wasm_uri": "file:///absolute/path/to/my-policy.wasm",
  "sha256": "<sha256-hex>",
  "supported_stages": ["pre_upstream"],
  "config_schema": {
    "type": "object",
    "required": ["message"],
    "properties": {
      "message": { "type": "string" }
    },
    "additionalProperties": false
  },
  "default_config": {
    "message": "hello"
  }
}
```

## 2) Define Policy Actions in Effective Config

Use `default_config` + route `params` to produce effective action payload. Supported action fields:

- `request_headers`
- `request_rewrite`
- `upstream_hint`
- `direct_response`
- `request_body_patch`
- `response_headers`

Compatibility alias:

- `headers` is accepted as alias of `request_headers`.

If effective config cannot be parsed into valid actions, DP fails closed (request returns `500`).

## 3) Provide a Policy Artifact

DP requires a `wasm_uri` and `sha256` for each policy artifact in snapshots.

- Supported URI schemes in runtime:
  - `file://`
  - `http://`
  - `https://`
- `oci://` is planned but currently not implemented in runtime loader.

Compute SHA-256:

```bash
shasum -a 256 /absolute/path/to/my-policy.wasm
```

Use that digest in policy registration.

## 4) Register Policy and Attach to a Route

Register policy:

```bash
curl -s -X POST http://127.0.0.1:8081/policies \
  -H 'Content-Type: application/json' \
  -d @policy.json
```

Attach to route (`params` overrides `default_config`):

```json
{
  "id": "route-with-my-policy",
  "match": { "path_prefix": "/v1/test", "method": ["GET"] },
  "upstreams": [{ "url": "http://127.0.0.1:18085" }],
  "policies": [
    {
      "stage": "pre_upstream",
      "id": "my-policy",
      "version": "1.0.0",
      "params": {
        "request_headers": [
          { "name": "x-my-policy", "value": "applied", "overwrite": true }
        ]
      }
    }
  ]
}
```

## 5) Test with Cucumber

Add a feature file under `gateway-it/features/`, following `gateway-it/features/policy_add_header.feature`.

Use placeholders supported by test world:

- `{{policy_add_header_wasm_uri}}`
- `{{policy_add_header_sha256}}`
- `{{upstream_url}}`
- `{{control_plane}}`
- `{{gateway}}`

For a new custom policy, add equivalent placeholders in `gateway-it/tests/cucumber.rs` if needed.

### Process-based integration test

```bash
make it-local
```

Or split flow:

```bash
make it-local-up
make it-local-test
make it-local-down
```

### Docker-based integration test

```bash
docker compose -f docker-compose.yml -f docker-compose.it.yml build
docker compose -f docker-compose.yml -f docker-compose.it.yml run --rm gateway-it
```

## 6) Recommended Test Scenarios

- Valid policy registration (`201`).
- Invalid schema/default config (`422`).
- Route attach with valid params (`201`).
- Route attach with invalid params (`422`).
- Positive request path shows policy effect at upstream.
- Missing/unsupported executor returns `500` (fail-closed).

## Troubleshooting

- Snapshot not applying in DP:
  - Check `target/it-local/logs/gateway-dp.log` for `cp sync error`.
  - Most common cause: invalid `wasm_uri` path or SHA mismatch.
- Route exists in CP but no match in DP:
  - If snapshot preload fails, DP keeps previous active snapshot.
  - Fix policy artifact issue first, then republish route/policy.

## Forward Path (Component Model Execution)

When runtime is switched to true WIT/Component execution:

- Keep CP contract (`config_schema`, `default_config`, route `params`) unchanged.
- Move policy-specific logic from host match branches into Wasm component implementations.
- Keep cucumber coverage unchanged except artifact/tooling setup.
