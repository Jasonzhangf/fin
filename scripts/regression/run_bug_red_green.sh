#!/usr/bin/env bash
set -euo pipefail
if [[ $# -lt 2 ]]; then
  echo "usage: $0 <bug-id> <red-command>"
  exit 2
fi
BUG_ID="$1"; shift
RED_CMD="$*"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/reports/regression/bug-repro/$BUG_ID"
mkdir -p "$OUT"

echo "[RED] $RED_CMD"
set +e
bash -lc "$RED_CMD" >"$OUT/red.log" 2>&1
RED_CODE=$?
set -e
if [[ $RED_CODE -eq 0 ]]; then
  echo "RED test did not fail; cannot proceed" | tee "$OUT/status.txt"
  exit 1
fi

echo "RED_OK" > "$OUT/status.txt"
echo "Now apply fix, then run: scripts/regression/run_bug_green.sh $BUG_ID <green-command>"
