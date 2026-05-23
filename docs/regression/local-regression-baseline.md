# Local Regression Baseline (P0)

## Scope (P0 only)
- No Android client dependency.
- Run local regression in 3 levels:
  - L0: offline deterministic checks (no network/provider required)
  - L1: local daemon/ws event-path checks (localhost only)
  - L2: optional live-provider checks (opt-in)

## Gates

### Gate G0 (entry + offline)
- Command: `scripts/regression/run_local_regression.sh`
- Must default to run L0+L1 only.
- Must support `--with-live` to include L2.

### Gate G1 (basic reasoning)
- `fin-cli config-check <user.toml>`
- `fin-cli transcript-session <user.toml> <fixture>`
- `fin-cli debug-projection <user.toml> "<input>"`

### Gate G2 (event parse/render contracts)
- Contract file checks:
  - `docs/contracts/tool-execution-record-contract.md`
  - `docs/contracts/reasoning-view-contract.md`
- Local script validates required event kinds:
  - tool start/progress/completed/error
  - missing-field raw JSON fallback branch exists

### Gate G3 (connection states)
- Validate mapping states in mobile-shell parser code:
  - 未连接
  - 已连接
  - 配置加载失败

### Gate G4 (receipt)
- Write:
  - `reports/regression/local-regression-status.json`
  - `reports/regression/local-regression-summary.md`

## Evidence Index Template
- gate name
- command
- status
- artifact path
- timestamp
