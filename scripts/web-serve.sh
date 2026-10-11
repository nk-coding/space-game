#!/usr/bin/env bash
# Dev web build with live reload, reachable from the host browser: http://10.77.6.2:8081
# (port 8080 is Niklas's copy, served by the lead from /home/devbox/workspace/serve; PORT=... overrides)
# Usage: scripts/web-serve.sh [--release] [extra trunk args]
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$(dirname "$0")/../crates/game"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
if [[ "${1:-}" == "--release" ]]; then
  shift
  exec trunk serve --release --cargo-profile "${WASM_PROFILE:-release}" --address 0.0.0.0 --port "${PORT:-8081}" "$@"
fi
exec trunk serve --address 0.0.0.0 --port "${PORT:-8081}" "$@"
