#!/usr/bin/env bash
# probe-cache-hit.sh — Validate prompt cache hit rate across simulated turns.
#
# Uses the fin provider to send N turns and checks cached_tokens from each response.
# Requires: fin-cli binary, valid provider config.
#
# Usage: ./scripts/probe-cache-hit.sh [num_turns] [threshold_pct]
#   num_turns: number of turns to simulate (default: 5)
#   threshold_pct: minimum warm-turn cache hit % (default: 50)

set -euo pipefail

TURNS="${1:-5}"
THRESHOLD="${2:-50}"
FIN_CLI="${FIN_CLI:-./rust/target/debug/fin-cli}"
RUNTIME_HOME="${FIN_RUNTIME_HOME:-$HOME/.fin}"

if [ ! -x "$FIN_CLI" ]; then
  echo "ERROR: fin-cli not found at $FIN_CLI. Build first: cd rust && cargo build --bin fin-cli"
  exit 1
fi

echo "=== Cache Hit Probe ==="
echo "turns=$TURNS threshold=${THRESHOLD}%"
echo "runtime_home=$RUNTIME_HOME"
echo ""

HITS=()
MISSES=()
RATIOS=()

for i in $(seq 1 "$TURNS"); do
  echo -n "turn-$i: "

  # Build a test session message
  MSG="Cache probe turn $i: $(head -c 200 /dev/urandom | base64 | tr -d '\n' | head -c 200)"

  # Run a single turn via the provider
  OUTPUT=$("$FIN_CLI" provider-live-smoke "$RUNTIME_HOME/config/user.toml" 2>/dev/null || echo "skip")

  if echo "$OUTPUT" | grep -q "cached_tokens"; then
    CACHED=$(echo "$OUTPUT" | grep -oP 'cached_tokens[=:]\s*\K[0-9]+' | head -1 || echo "0")
    PROMPT=$(echo "$OUTPUT" | grep -oP 'prompt_tokens[=:]\s*\K[0-9]+' | head -1 || echo "1")
    HIT_PCT=0
    if [ "$PROMPT" -gt 0 ] 2>/dev/null; then
      HIT_PCT=$((CACHED * 100 / PROMPT))
    fi
    echo "cached=$CACHED/$PROMPT hit=${HIT_PCT}%"
    HITS+=("$CACHED")
    MISSES+=$((PROMPT - CACHED))
    RATIOS+=("$HIT_PCT")
  else
    echo "no cached_tokens in response (provider may not support cache)"
    HITS+=(0)
    MISSES+=(0)
    RATIOS+=(0)
  fi
done

echo ""
echo "=== Summary ==="
WARM_HITS=0
WARM_COUNT=0
for i in "${!RATIOS[@]}"; do
  echo "  turn-$((i+1)): ${RATIOS[$i]}%"
  if [ "$i" -gt 0 ]; then
    WARM_HITS=$((WARM_HITS + RATIOS[$i]))
    WARM_COUNT=$((WARM_COUNT + 1))
  fi
done

if [ "$WARM_COUNT" -gt 0 ]; then
  AVG=$((WARM_HITS / WARM_COUNT))
  echo ""
  echo "warm-turn average: ${AVG}%"
  if [ "$AVG" -ge "$THRESHOLD" ]; then
    echo "PASS: warm-turn average ${AVG}% >= ${THRESHOLD}%"
    exit 0
  else
    echo "FAIL: warm-turn average ${AVG}% < ${THRESHOLD}%"
    exit 1
  fi
else
  echo "SKIP: not enough turns to evaluate"
  exit 0
fi
