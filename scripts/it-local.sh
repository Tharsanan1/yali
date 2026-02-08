#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/target/it-local"
RUN_DIR="${TARGET_DIR}/run"
LOG_DIR="${TARGET_DIR}/logs"

CP_BIN="${ROOT_DIR}/target/debug/gateway-cp"
DP_BIN="${ROOT_DIR}/target/debug/gateway-dp"

CP_PORT="${IT_CP_PORT:-18081}"
DP_PORT="${IT_DP_PORT:-18080}"
CP_GRPC_PORT="${IT_CP_GRPC_PORT:-19090}"
UPSTREAM_PORT="${IT_UPSTREAM_PORT:-18085}"
LOG_LEVEL="${IT_LOCAL_LOG_LEVEL:-debug}"

CP_BASE_URL="http://127.0.0.1:${CP_PORT}"
DP_BASE_URL="http://127.0.0.1:${DP_PORT}"
UPSTREAM_URL="http://127.0.0.1:${UPSTREAM_PORT}"

CP_PID_FILE="${RUN_DIR}/gateway-cp.pid"
DP_PID_FILE="${RUN_DIR}/gateway-dp.pid"
UPSTREAM_PID_FILE="${RUN_DIR}/upstream.pid"

usage() {
  cat <<EOF
Usage: $0 <up|test|down|run>

Commands:
  up    Build binaries (unless IT_LOCAL_SKIP_BUILD=1), start upstream + CP + DP
  test  Run cucumber integration tests against local processes
  down  Stop local integration processes
  run   up + test + down
EOF
}

ensure_dirs() {
  mkdir -p "${RUN_DIR}" "${LOG_DIR}"
}

is_running() {
  local pid="$1"
  kill -0 "${pid}" 2>/dev/null
}

read_pid() {
  local file="$1"
  if [[ -f "${file}" ]]; then
    cat "${file}"
  fi
}

assert_not_running() {
  local file="$1"
  local name="$2"
  local pid
  pid="$(read_pid "${file}")"
  if [[ -n "${pid}" ]] && is_running "${pid}"; then
    echo "${name} is already running with PID ${pid}. Run '$0 down' first." >&2
    exit 1
  fi
  rm -f "${file}"
}

wait_http_success() {
  local url="$1"
  local timeout_s="$2"
  local end=$((SECONDS + timeout_s))
  while (( SECONDS < end )); do
    if curl -fsS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  echo "Timed out waiting for successful response from ${url}" >&2
  return 1
}

wait_http_ready() {
  local url="$1"
  local timeout_s="$2"
  local end=$((SECONDS + timeout_s))
  while (( SECONDS < end )); do
    if curl -sS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  echo "Timed out waiting for reachable endpoint ${url}" >&2
  return 1
}

wait_tcp_ready() {
  local host="$1"
  local port="$2"
  local timeout_s="$3"
  local end=$((SECONDS + timeout_s))
  while (( SECONDS < end )); do
    if (echo >/dev/tcp/"${host}"/"${port}") >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  echo "Timed out waiting for TCP ${host}:${port}" >&2
  return 1
}

build_binaries() {
  if [[ "${IT_LOCAL_SKIP_BUILD:-0}" == "1" ]]; then
    return 0
  fi

  "${ROOT_DIR}/scripts/build-policy-artifacts.sh"
  cargo build -p gateway-cp -p gateway-dp -p gateway-it
}

ensure_binaries_exist() {
  if [[ ! -x "${CP_BIN}" ]]; then
    echo "Missing binary: ${CP_BIN}. Run 'make it-local-build' first." >&2
    exit 1
  fi
  if [[ ! -x "${DP_BIN}" ]]; then
    echo "Missing binary: ${DP_BIN}. Run 'make it-local-build' first." >&2
    exit 1
  fi
}

start_upstream() {
  nohup python3 "${ROOT_DIR}/scripts/upstream_echo.py" \
    --bind "127.0.0.1:${UPSTREAM_PORT}" \
    >"${LOG_DIR}/upstream.log" 2>&1 </dev/null &
  echo $! >"${UPSTREAM_PID_FILE}"
}

start_cp() {
  nohup env \
    RUST_LOG="${LOG_LEVEL}" \
    GATEWAY_CP_CONFIG="${ROOT_DIR}/config/control-plane.example.toml" \
    GATEWAY_CP__BIND="127.0.0.1:${CP_PORT}" \
    GATEWAY_CP__GRPC_BIND="127.0.0.1:${CP_GRPC_PORT}" \
    GATEWAY_CP__DATABASE_URL="sqlite://${TARGET_DIR}/control-plane.db" \
    GATEWAY_CP__LOGGING__LEVEL="${LOG_LEVEL}" \
    "${CP_BIN}" \
    >"${LOG_DIR}/gateway-cp.log" 2>&1 </dev/null &
  echo $! >"${CP_PID_FILE}"
}

start_dp() {
  nohup env \
    RUST_LOG="${LOG_LEVEL}" \
    GATEWAY_DP_CONFIG="${ROOT_DIR}/config/gateway.example.toml" \
    GATEWAY_DP__LISTENER__BIND="127.0.0.1:${DP_PORT}" \
    GATEWAY_DP__CONTROL_PLANE__GRPC_ENDPOINT="http://127.0.0.1:${CP_GRPC_PORT}" \
    GATEWAY_DP__LOGGING__LEVEL="${LOG_LEVEL}" \
    "${DP_BIN}" \
    >"${LOG_DIR}/gateway-dp.log" 2>&1 </dev/null &
  echo $! >"${DP_PID_FILE}"
}

up() {
  ensure_dirs
  build_binaries
  ensure_binaries_exist

  assert_not_running "${UPSTREAM_PID_FILE}" "upstream"
  assert_not_running "${CP_PID_FILE}" "gateway-cp"
  assert_not_running "${DP_PID_FILE}" "gateway-dp"

  rm -f "${TARGET_DIR}/control-plane.db"

  start_upstream
  start_cp
  start_dp

  wait_http_success "${UPSTREAM_URL}" 30
  wait_http_success "${CP_BASE_URL}/health" 30
  wait_tcp_ready "127.0.0.1" "${CP_GRPC_PORT}" 30
  wait_http_ready "${DP_BASE_URL}" 30

  echo "Local IT stack is ready."
  echo "CP: ${CP_BASE_URL}"
  echo "DP: ${DP_BASE_URL}"
  echo "Upstream: ${UPSTREAM_URL}"
}

test_it() {
  GATEWAY_IT_CP_BASE_URL="${CP_BASE_URL}" \
  GATEWAY_IT_DP_BASE_URL="${DP_BASE_URL}" \
  GATEWAY_IT_UPSTREAM_URL="${UPSTREAM_URL}" \
  GATEWAY_IT_UPSTREAM_CHECK_URL="${UPSTREAM_URL}" \
  cargo test -p gateway-it --test cucumber
}

stop_by_pid_file() {
  local file="$1"
  local name="$2"
  local pid

  pid="$(read_pid "${file}")"
  if [[ -z "${pid}" ]]; then
    return 0
  fi

  if is_running "${pid}"; then
    kill -9 "${pid}" 2>/dev/null || true
    wait "${pid}" 2>/dev/null || true
  fi

  rm -f "${file}"
  echo "Stopped ${name}"
}

down() {
  stop_by_pid_file "${DP_PID_FILE}" "gateway-dp"
  stop_by_pid_file "${CP_PID_FILE}" "gateway-cp"
  stop_by_pid_file "${UPSTREAM_PID_FILE}" "upstream"
}

run_all() {
  trap 'down' EXIT
  up
  test_it
}

main() {
  local cmd="${1:-}"
  case "${cmd}" in
    up)
      up
      ;;
    test)
      test_it
      ;;
    down)
      down
      ;;
    run)
      run_all
      ;;
    *)
      usage
      exit 1
      ;;
  esac
}

main "$@"
