#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$ROOT_DIR/rust"
USER_TOML="${1:-${FIN_USER_TOML:-$HOME/.fin/config/user.toml}}"
FIN_HOME="${FIN_HOME:-$HOME/.fin}"
GLOBAL_BIN_DIR="${FIN_GLOBAL_BIN_DIR:-$HOME/.local/bin}"
BUILD_VERSION="${FIN_BUILD_VERSION:-}"

mkdir -p "$FIN_HOME/config" "$GLOBAL_BIN_DIR"

if [[ ! -f "$USER_TOML" ]]; then
  cat > "$USER_TOML" <<'TOML'
default_provider = "openai"

[runtime]
runtime_home = "~/.fin"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.openai.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
TOML
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
  "$SCRIPT_DIR/bootstrap-macos-permissions.sh"
fi

cd "$RUST_DIR"
echo "Building fin-cli release..."
cargo build -p fin-cli --release

FIN_CLI="$RUST_DIR/target/release/fin-cli"
if [[ ! -x "$FIN_CLI" ]]; then
  echo "Build failed: fin-cli binary not found at $FIN_CLI" >&2
  exit 1
fi

echo "Running canonical fin install-dev flow..."
if [[ -n "$BUILD_VERSION" ]]; then
  "$FIN_CLI" install-dev "$USER_TOML" "$BUILD_VERSION"
else
  "$FIN_CLI" install-dev "$USER_TOML"
fi

INSTALLED_FIN="$FIN_HOME/bin/fin"
if [[ ! -x "$INSTALLED_FIN" ]]; then
  echo "Install failed: expected executable not found at $INSTALLED_FIN" >&2
  exit 1
fi

ln -sfn "$INSTALLED_FIN" "$GLOBAL_BIN_DIR/fin"
echo "Global fin symlink: $GLOBAL_BIN_DIR/fin -> $INSTALLED_FIN"

echo "Restarting fin daemon through fin CLI..."
"$INSTALLED_FIN" stop "$USER_TOML" >/dev/null 2>&1 || true
"$INSTALLED_FIN" start "$USER_TOML"

if curl -fsS --max-time 2 http://127.0.0.1:4040/ >/dev/null 2>&1; then
  echo "Daemon running on :4040"
else
  echo "Warning: daemon health check did not respond on :4040; inspect $FIN_HOME/logs/runtime/headless-daemon.log" >&2
fi

echo "Install complete. Add $GLOBAL_BIN_DIR to PATH if 'fin' is not found globally."
