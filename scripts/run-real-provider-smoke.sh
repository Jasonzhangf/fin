#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

run_id="${1:-test-live-provider-$(date +%Y%m%d-%H%M%S)}"
if [[ "${run_id}" != test-* ]]; then
  run_id="test-${run_id}"
fi

base_home="${HOME}/.fin"
run_root="${base_home}/harness/runs/${run_id}"
runtime_home="${run_root}/runtime-home"
user_toml="${run_root}/user.test.toml"
probe_report="${run_root}/anthropic-probe.json"
probe_log="${run_root}/provider-probe.log"
smoke_log="${run_root}/provider-live-smoke.log"

mkdir -p "${run_root}"
"${REPO_ROOT}/scripts/generate-test-user-toml.py" \
  --output "${user_toml}" \
  --scope-label real-provider-test >/dev/null

export FIN_RUNTIME_HOME_OVERRIDE="${runtime_home}"
export FIN_SESSION_NAMESPACE="${run_id}"

echo "[fin] real provider smoke"
echo "  run_id=${run_id}"
echo "  user_toml=${user_toml}"
echo "  runtime_home=${runtime_home}"

run_with_retry() {
  local label="$1"
  local log_path="$2"
  shift 2
  local attempt=1
  local max_attempts=5
  while true; do
    rm -f "$log_path"
    set +e
    "$@" >"$log_path" 2>&1
    local code=$?
    set -e
    if [[ $code -eq 0 ]]; then
      cat "$log_path"
      return 0
    fi
    if rg -q "usage limit exceeded|weekly usage limit reached|quota|429|rate limit" "$log_path"; then
      if [[ $attempt -lt $max_attempts ]]; then
        local sleep_secs=$((2 ** (attempt - 1)))
        echo "[fin] ${label} transient quota/rate-limit on attempt ${attempt}; retrying in ${sleep_secs}s" >&2
        attempt=$((attempt + 1))
        sleep "$sleep_secs"
        continue
      fi
    fi
    cat "$log_path"
    return $code
  done
}

run_with_retry probe "$probe_log" \
  python3 "${REPO_ROOT}/scripts/probe-anthropic-provider.py" \
    --user-toml "${user_toml}" \
    --report "${probe_report}"

run_with_retry smoke "$smoke_log" \
  cargo run -p fin-cli --manifest-path rust/Cargo.toml -- provider-live-smoke "${user_toml}"

echo "[fin] live smoke receipt: ${run_root}/provider-live-smoke-report.json"
