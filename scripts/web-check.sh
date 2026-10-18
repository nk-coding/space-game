#!/usr/bin/env bash
# One bounded web check, for agents: build the client once, serve it on a free port, open it in headless
# Chromium, save a screenshot, print the browser console, stop the server. Every step has a time limit, so
# this can never hang (see CLAUDE.md, "Wait loops must end"). Prefer this over `trunk serve` + log polling.
#
# Usage: scripts/web-check.sh [options]
#   --release            optimised build (default: dev build, faster to compile)
#   --params '?a=1&b=2'  URL query string for the game (e.g. '?scene=sandbox', '?scene=sandbox&load=5', '?scene=showcase&hud=0';
#                        none: the home screen, where the game starts)
#   --wait MS            how long the page runs before the screenshot (default 20000; SwiftShader is slow)
#   --size WxH           viewport (default 1280x720)
#   --out FILE           screenshot path (default: target/web-check/shot.png)
#   --build-timeout S    build time limit in seconds (default 1500)
# Exit code: non-zero on build failure, timeout, or a wasm panic / page error (3). Look at the PNG with the Read tool.
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RELEASE=() PARAMS="" WAIT=20000 SIZE=1280x720 BUILD_TIMEOUT=1500
OUT="$ROOT/target/web-check/shot.png"
while [ $# -gt 0 ]; do
  case "$1" in
    --release) RELEASE=(--release --cargo-profile release) ;;
    --params) PARAMS="$2"; shift ;;
    --wait) WAIT="$2"; shift ;;
    --size) SIZE="$2"; shift ;;
    --out) OUT="$2"; shift ;;
    --build-timeout) BUILD_TIMEOUT="$2"; shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done
DIST="$ROOT/target/web-check/dist-$$"
mkdir -p "$(dirname "$OUT")" "$DIST"
SERVER_PID=""
cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
  rm -rf "$DIST"
}
trap cleanup EXIT

echo "== build (limit ${BUILD_TIMEOUT}s)"
cd "$ROOT/crates/client/game"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
set +e
timeout "$BUILD_TIMEOUT" trunk build "${RELEASE[@]}" --dist "$DIST" 2>&1 | tail -25
status=${PIPESTATUS[0]}
set -e
[ "$status" -eq 0 ] || { echo "== build FAILED (exit $status; 124 = timeout)"; exit "$status"; }

PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$DIST" >/dev/null 2>&1 &
SERVER_PID=$!
timeout 15 bash -c "until (exec 3<>/dev/tcp/127.0.0.1/$PORT) 2>/dev/null; do sleep 0.2; done" \
  || { echo "== server did not start"; exit 1; }

URL="http://127.0.0.1:$PORT/$PARAMS"
echo "== browser: $URL (wait ${WAIT} ms)"
W=${SIZE%x*} H=${SIZE#*x}
LOG="$DIST.browser.log"
set +e
timeout $(( WAIT / 1000 + 180 )) node /home/devbox/workspace/tools/headless/shot.mjs "$URL" "$OUT" "$WAIT" "$W" "$H" >"$LOG" 2>&1
status=$?
set -e
# Console output without the long wasm stack frames; panics and page errors go first so they can't be missed.
problems=$(grep -E "panicked at|\[pageerror\]|^error: " "$LOG" | head -20 || true)
{ grep -vE "^\s+at " "$LOG" || true; } | tail -40
rm -f "$LOG"
echo "== screenshot: $OUT (browser exit $status)"
if [ -n "$problems" ]; then
  echo "== PROBLEMS (wasm panic / page error):"
  echo "$problems"
  [ "$status" -eq 0 ] && status=3
fi
exit "$status"
