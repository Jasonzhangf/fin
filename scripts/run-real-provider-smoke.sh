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
python3 "${REPO_ROOT}/scripts/probe-anthropic-provider.py" \
  --user-toml "${user_toml}" \
  --report "${probe_report}"

cargo run -p fin-cli --manifest-path rust/Cargo.toml -- provider-live-smoke "${user_toml}"

echo "[fin] live smoke receipt: ${run_root}/provider-live-smoke-report.json"
