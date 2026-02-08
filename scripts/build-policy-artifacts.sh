#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${ROOT_DIR}"

cargo build --release --target wasm32-wasip2 -p add-header-policy

cp \
  "${ROOT_DIR}/target/wasm32-wasip2/release/add_header_policy.wasm" \
  "${ROOT_DIR}/policies/add-header/add-header.wasm"

echo "Built policy artifact: policies/add-header/add-header.wasm"
