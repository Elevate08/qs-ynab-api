#!/usr/bin/env bash
# Capture README screenshots against demo fixture data.
#
# Restarts the Omarchy shell with demo fixtures ahead on PATH / QSYNAB_CLI_PATH.
# Real credentials/tokens are untouched and the shell is restored upon exit.
#
# Usage:
#   ./demo/capture.sh [output-dir]     (default: docs/screenshots)

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$REPO/docs/screenshots}"
IPC=(qs -p /usr/share/omarchy/shell/shell.qml ipc call io.github.elevate08.ynab-glance)

for tool in grim magick hyprctl quickshell /usr/bin/python3; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

mkdir -p "$OUT"

restore() {
  echo "Restoring the real Omarchy shell..."
  pkill -f "quickshell -n -p /usr/share/omarchy/shell" 2>/dev/null || true
  sleep 1
  omarchy restart shell >/dev/null 2>&1 || true
}
trap restore EXIT INT TERM

start_shell() {
  local state="$1"
  echo "Starting shell with fixture data ($state)..."
  pkill -f "quickshell -n -p /usr/share/omarchy/shell" 2>/dev/null || true
  sleep 1
  PATH="$REPO/demo/bin:$PATH" \
  QSYNAB_CLI_PATH="$REPO/demo/bin/ynab-cli" \
  QSYNAB_DEMO_STATUS="$state" \
    nohup quickshell -n -p /usr/share/omarchy/shell >/dev/null 2>&1 &
  sleep 6
  # Park pointer away from UI to avoid unwanted hover states
  hyprctl dispatch movecursor 100 100 >/dev/null 2>&1 || true
}

shot() { # shot <name>
  sleep 1
  grim "$OUT/.raw.png"
  local box err
  if box="$(/usr/bin/python3 "$REPO/demo/find_panel.py" "$OUT/.raw.png" 2>/tmp/find_panel.err)"; then
    magick "$OUT/.raw.png" -crop "$box" +repage "$OUT/$1.png"
    echo "  wrote $1.png ($box)"
  else
    err="$(cat /tmp/find_panel.err 2>/dev/null || true)"
    echo "  FALLBACK $1: cropping panel region 720x960+1392+50 ($err)"
    magick "$OUT/.raw.png" -crop 720x960+1392+50 +repage "$OUT/$1.png"
    echo "  wrote $1.png (exact panel crop)"
  fi
  rm -f /tmp/find_panel.err "$OUT/.raw.png"
}

# --- 1. Unauthenticated / Onboarding Screen ---
start_shell unauthenticated
"${IPC[@]}" open >/dev/null 2>&1 || true; sleep 3
shot 06-onboarding
"${IPC[@]}" close >/dev/null 2>&1 || true

# --- 2. Unlocked, Populated Vault Views ---
start_shell unlocked

# Tab 0: Buckets overview (with Ready to Assign banner & Overspent highlight)
"${IPC[@]}" tab 0 >/dev/null 2>&1 || true; sleep 3
shot 01-buckets

# Tab 1: Income & Age (with 6-Month Trend Graph)
"${IPC[@]}" tab 1 >/dev/null 2>&1 || true; sleep 3
shot 02-income-and-trends

# Tab 2: Spending Analysis (Pie Chart & Category Breakdown)
"${IPC[@]}" tab 2 >/dev/null 2>&1 || true; sleep 3
shot 03-spending-analysis

# Tab 2 Drill-down: Sub-Category Spending Breakdown
"${IPC[@]}" drilldown >/dev/null 2>&1 || true; sleep 3
shot 04-spending-drilldown

# Settings screen: Active budget selector, refresh slider, keyring security
"${IPC[@]}" settings >/dev/null 2>&1 || true; sleep 3
shot 05-settings

"${IPC[@]}" close >/dev/null 2>&1 || true

echo "Screenshots captured successfully into $OUT"
