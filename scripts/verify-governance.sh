#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

echo "=== Governance Check ==="

# 1. Required files exist
required=(
  "AGENTS.md"
  "docs/architecture/01-system-overview.md"
  "docs/architecture/02-layer-boundaries.md"
  "docs/architecture/08-testing-and-ci-strategy.md"
  "docs/architecture/44-runtime-error-center.md"
  "docs/architecture/45-runtime-module-inventory.md"
  "docs/architecture/46-pipeline-unique-type-and-error-chain.md"
  "skills/fin-general-dev/SKILL.md"
  "skills/fin-architecture/SKILL.md"
  "skills/fin-testing-harness/SKILL.md"
  "skills/fin-runtime-debug/SKILL.md"
  "rust/Cargo.toml"
  "scripts/check-code-line-limit.py"
  "scripts/line-limit-whitelist.txt"
  "scripts/probe-anthropic-provider.py"
)

missing=0
for path in "${required[@]}"; do
  if [[ ! -f "$path" ]]; then
    echo "FAIL: missing required file: $path" >&2
    missing=1
  fi
done
if [[ $missing -eq 1 ]]; then
  echo "FAIL: required files missing" >&2
  exit 1
fi
echo "OK: required files present"

# 2. Line-limit gate
echo "Running line-limit gate..."
if python3 scripts/check-code-line-limit.py; then
  echo "OK: line-limit gate passed"
else
  echo "FAIL: line-limit gate failed" >&2
  exit 1
fi

# 3. Pipeline routing doc exists
if [[ -f "docs/architecture/46-pipeline-unique-type-and-error-chain.md" ]]; then
  echo "OK: pipeline unique-type routing doc exists"
else
  echo "FAIL: docs/architecture/46-pipeline-unique-type-and-error-chain.md missing" >&2
  exit 1
fi

# 4. Runtime inventory mentions actual domain dirs
for domain in model prompt; do
  if grep -q "|\`${domain}\`" docs/architecture/45-runtime-module-inventory.md; then
    echo "OK: inventory includes domain $domain"
  else
    echo "FAIL: inventory missing domain $domain" >&2
    exit 1
  fi
done

echo "=== Governance check passed ==="
