#!/usr/bin/env bash
# Dev web build with live reload, reachable from the host browser: http://10.77.6.2:8080
# Usage: scripts/web-serve.sh [--release] [extra trunk args]
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$(dirname "$0")/../crates/game"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
if [[ "${1:-}" == "--release" ]]; then
  shift
  exec trunk serve --release --cargo-profile wasm-release --address 0.0.0.0 --port 8080 "$@"
fi
exec trunk serve --address 0.0.0.0 --port 8080 "$@"
