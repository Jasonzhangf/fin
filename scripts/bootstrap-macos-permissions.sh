#!/usr/bin/env bash
set -euo pipefail

# First-install macOS permission bootstrap for fin.
# macOS TCC permissions cannot be silently granted by a CLI. This script triggers
# the relevant prompts/panes once, records that bootstrap was offered, and leaves
# final approval to the user in System Settings.

if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

FIN_HOME="${FIN_HOME:-$HOME/.fin}"
MARKER_DIR="$FIN_HOME/install"
MARKER="$MARKER_DIR/macos-permissions-bootstrap.json"
FORCE="${FIN_FORCE_PERMISSION_BOOTSTRAP:-0}"

if [[ -f "$MARKER" && "$FORCE" != "1" ]]; then
  echo "macOS permission bootstrap already offered: $MARKER"
  exit 0
fi

mkdir -p "$MARKER_DIR" "$FIN_HOME/logs/install"
LOG="$FIN_HOME/logs/install/macos-permissions-bootstrap.log"
: > "$LOG"

log() {
  printf '%s %s\n' "$(date '+%Y-%m-%dT%H:%M:%S%z')" "$*" | tee -a "$LOG"
}

open_privacy_pane() {
  local pane="$1"
  if open "x-apple.systempreferences:com.apple.preference.security?${pane}" >>"$LOG" 2>&1; then
    log "opened System Settings privacy pane: ${pane}"
  else
    log "warning: failed to open privacy pane: ${pane}"
  fi
}

log "starting first-install macOS permission bootstrap"

# Trigger Automation / Apple Events prompt for the invoking terminal/app when needed.
osascript -e 'tell application "System Events" to count processes' >>"$LOG" 2>&1 || true

# Open panes fin-adjacent local automation commonly needs. Opening is explicit and
# truthful: the user must approve fin/Terminal/iTerm/Codex in System Settings.
open_privacy_pane Privacy_Accessibility
open_privacy_pane Privacy_ScreenCapture
open_privacy_pane Privacy_ListenEvent
open_privacy_pane Privacy_AllFiles

timestamp="$(date '+%Y-%m-%dT%H:%M:%S%z')"
cat > "$MARKER" <<JSON
{
  "status": "offered",
  "offered_at": "${timestamp}",
  "log_path": "${LOG}",
  "note": "macOS TCC approval requires user confirmation in System Settings; fin opened the required panes once during first install. Set FIN_FORCE_PERMISSION_BOOTSTRAP=1 to show them again."
}
JSON

log "permission bootstrap marker written: $MARKER"
log "If prompts continue, approve the invoking app and fin binary in the opened privacy panes."
