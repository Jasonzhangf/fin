#!/usr/bin/env bash
set -euo pipefail
if [[ $# -lt 2 ]]; then
  echo "usage: $0 <bug-id> <green-command>"
  exit 2
fi
BUG_ID="$1"; shift
GREEN_CMD="$*"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/reports/regression/bug-repro/$BUG_ID"
mkdir -p "$OUT"

echo "[GREEN] $GREEN_CMD"
bash -lc "$GREEN_CMD" >"$OUT/green.log" 2>&1

bash "$ROOT/scripts/regression/run_local_regression.sh" >"$OUT/full-regression.log" 2>&1

echo "GREEN_OK" >> "$OUT/status.txt"
echo "done: $OUT"
