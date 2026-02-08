# Custom Policy Authoring and Testing

This guide shows how to build, register, and verify a custom policy in the current gateway.

## Runtime Contract (Current)

- Control plane validates policy metadata and configuration with JSON Schema.
- Route policy `params` are merged with policy `default_config` in CP.
- CP publishes `effective_config_json` in gRPC snapshots.
- DP preloads policy artifacts (`wasm_uri` + `sha256`) and executes policies via Wasmtime Component Model (`policy-sdk/wit/policy.wit`).
- DP runs attached policies in route order for `pre_upstream`.
- Fail-closed: policy load or execution errors return `500`.

## 1) Implement a Policy Component

Create a Rust crate that exports the WIT world `pre-upstream-policy`.

Reference implementation:
- `/Users/tharsanan/Documents/Projects/yali/policies/add-header/src/lib.rs`

WIT source:
- `/Users/tharsanan/Documents/Projects/yali/policy-sdk/wit/policy.wit`

Required guest export:
- `evaluate-pre-upstream(method, path, host, headers-json, effective-config-json) -> result<policy-decision, string>`

`policy-decision` supports:
- `request-headers`
- `request-rewrite`
- `upstream-hint`
- `direct-response`
- `request-body-patch-json`
- `response-headers`

Current DP execution support in `pre_upstream`:
- supported now: `request-headers`, `request-rewrite`, `upstream-hint`
- currently rejected (500): `direct-response`, `request-body-patch-json`, `response-headers`

## 2) Build the WASM Artifact

Build all policy artifacts:

```bash
./scripts/build-policy-artifacts.sh
```

Current output:
- `/Users/tharsanan/Documents/Projects/yali/policies/add-header/add-header.wasm`

Compute digest:

```bash
shasum -a 256 /Users/tharsanan/Documents/Projects/yali/policies/add-header/add-header.wasm
```

## 3) Register Policy in Control Plane

Example `POST /policies`:

```json
{
  "id": "add-header",
  "version": "1.0.0",
  "wasm_uri": "file:///Users/tharsanan/Documents/Projects/yali/policies/add-header/add-header.wasm",
  "sha256": "<sha256-hex>",
  "supported_stages": ["pre_upstream"],
  "config_schema": {
    "type": "object",
    "required": ["request_headers"],
    "properties": {
      "request_headers": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["name", "value", "overwrite"],
          "properties": {
            "name": { "type": "string", "minLength": 1 },
            "value": { "type": "string" },
            "overwrite": { "type": "boolean" }
          },
          "additionalProperties": false
        },
        "minItems": 1
      }
    },
    "additionalProperties": false
  },
  "default_config": {
    "request_headers": [
      { "name": "x-policy", "value": "default", "overwrite": true }
    ]
  }
}
```

## 4) Attach Policy to a Route

Example route payload with policy params:

```json
{
  "id": "route-with-policy",
  "match": { "path_prefix": "/v1/test", "method": ["GET"] },
  "upstreams": [{ "url": "http://127.0.0.1:18085" }],
  "policies": [
    {
      "stage": "pre_upstream",
      "id": "add-header",
      "version": "1.0.0",
      "params": {
        "request_headers": [
          { "name": "x-policy", "value": "override", "overwrite": true }
        ]
      }
    }
  ]
}
```

CP computes:
- `effective_config = deep_merge(default_config, params)`

## 5) Test a Policy End to End

### Process mode (fastest for local iteration)

```bash
make it-local
```

Or split:

```bash
make it-local-up
make it-local-test
make it-local-down
```

### Docker mode

```bash
docker compose -f docker-compose.yml -f docker-compose.it.yml build
docker compose -f docker-compose.yml -f docker-compose.it.yml run --rm gateway-it
```

## 6) Add Cucumber Coverage for New Policies

Add feature files under:
- `/Users/tharsanan/Documents/Projects/yali/gateway-it/features`

Existing policy example:
- `/Users/tharsanan/Documents/Projects/yali/gateway-it/features/policy_add_header.feature`

Available placeholders in test docstrings:
- `{{policy_add_header_wasm_uri}}`
- `{{policy_add_header_sha256}}`
- `{{upstream_url}}`
- `{{control_plane}}`
- `{{gateway}}`

If you introduce additional artifacts, add equivalent placeholders in:
- `/Users/tharsanan/Documents/Projects/yali/gateway-it/tests/cucumber.rs`

## 7) URI Support Notes

DP loader currently supports:
- `file://`
- `http://`
- `https://`

`oci://` is not implemented yet in `policy-runtime` and currently fails preload.

## Troubleshooting

- Policy snapshot fails to apply:
  - Check `/Users/tharsanan/Documents/Projects/yali/target/it-local/logs/gateway-dp.log`
  - Typical causes: SHA mismatch, bad URI, guest instantiation failure.
- Route is present in CP but behavior not visible in DP:
  - DP keeps previous snapshot if new snapshot preload fails.
  - Fix preload issue and republish route/policy.
- Request returns `500` after policy attach:
  - Check for unsupported decision actions in `pre_upstream` (see runtime support section).
