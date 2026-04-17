#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

required=(
  "AGENTS.md"
  "docs/architecture/01-system-overview.md"
  "docs/architecture/02-layer-boundaries.md"
  "docs/architecture/08-testing-and-ci-strategy.md"
  "skills/fin-general-dev/SKILL.md"
  "skills/fin-architecture/SKILL.md"
  "skills/fin-testing-harness/SKILL.md"
  "skills/fin-runtime-debug/SKILL.md"
  "rust/Cargo.toml"
  "scripts/check-code-line-limit.py"
  "scripts/line-limit-whitelist.txt"
  "scripts/probe-anthropic-provider.py"
)

for path in "${required[@]}"; do
  if [[ ! -f "$path" ]]; then
    echo "missing required file: $path" >&2
    exit 1
  fi
done

echo "governance skeleton verified"