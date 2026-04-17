#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

run_id="${1:-test-m1-smoke-$(date +%Y%m%d-%H%M%S)}"
if [[ "${run_id}" != test-* ]]; then
  run_id="test-${run_id}"
fi

run_root="${HOME}/.fin/harness/runs/${run_id}"
runtime_home="${run_root}/runtime-home"
user_toml="${run_root}/user.test.toml"

mkdir -p "${run_root}"
"${REPO_ROOT}/scripts/generate-test-user-toml.py" \
  --output "${user_toml}" \
  --scope-label isolated-test >/dev/null

export FIN_RUNTIME_HOME_OVERRIDE="${runtime_home}"
export FIN_SESSION_NAMESPACE="${run_id}"

echo "[fin] isolated M1 smoke"
echo "  run_id=${run_id}"
echo "  user_toml=${user_toml}"
echo "  runtime_home=${runtime_home}"

cargo run -p fin-cli --manifest-path rust/Cargo.toml -- config-check "${user_toml}"
cargo run -p fin-cli --manifest-path rust/Cargo.toml -- runtime-demo "${user_toml}" "isolated smoke ${run_id}"
cargo run -p fin-cli --manifest-path rust/Cargo.toml -- debug-projection "${user_toml}" "isolated smoke ${run_id}"

echo "[fin] smoke artifacts written under ${run_root}"
