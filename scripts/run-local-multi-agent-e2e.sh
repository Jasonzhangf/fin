#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

MODE="${1:-live}"
RUN_ID="${2:-test-local-multi-agent-$(date +%Y%m%d-%H%M%S)}"
if [[ "${RUN_ID}" != test-* ]]; then
  RUN_ID="test-${RUN_ID}"
fi

RUN_ROOT="${HOME}/.fin/harness/runs/${RUN_ID}"
RUNTIME_HOME="${RUN_ROOT}/runtime-home"
USER_TOML="${RUN_ROOT}/user.test.toml"
PROJECT_CWD="${REPO_ROOT}"
REPORT_DIR="${REPO_ROOT}/reports/regression/local-multi-agent"
RECEIPT_COPY="${REPORT_DIR}/${RUN_ID}-receipt.json"

mkdir -p "${RUN_ROOT}" "${REPORT_DIR}"
"${REPO_ROOT}/scripts/generate-test-user-toml.py" \
  --output "${USER_TOML}" \
  --scope-label "local-multi-agent-${MODE}" >/dev/null

export FIN_RUNTIME_HOME_OVERRIDE="${RUNTIME_HOME}"
export FIN_SESSION_NAMESPACE="${RUN_ID}"

if [[ "${MODE}" == "static" ]]; then
  echo "error: static mode is no longer accepted for local multi-agent E2E validation; use live" >&2
  exit 64
fi
unset FIN_LOCAL_MULTI_AGENT_STATIC_LLM || true

echo "[fin] local multi-agent e2e"
echo "  mode=${MODE}"
echo "  run_id=${RUN_ID}"
echo "  runtime_home=${RUNTIME_HOME}"
echo "  user_toml=${USER_TOML}"
echo "  project_cwd=${PROJECT_CWD}"

cargo run -p fin-cli --manifest-path rust/Cargo.toml -- \
  local-multi-agent-harness "${USER_TOML}" "${PROJECT_CWD}"

RECEIPT_PATH="${RUNTIME_HOME}/receipts/local-multi-agent-lifecycle.json"
if [[ ! -f "${RECEIPT_PATH}" ]]; then
  echo "missing receipt: ${RECEIPT_PATH}" >&2
  exit 2
fi

cp "${RECEIPT_PATH}" "${RECEIPT_COPY}"
echo "[fin] copied receipt: ${RECEIPT_COPY}"
