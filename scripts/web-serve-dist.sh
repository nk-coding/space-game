#!/usr/bin/env bash
# Serves the last build in crates/game/dist without rebuilding (e.g. after scripts/web-build.sh).
set -euo pipefail
cd "$(dirname "$0")/../crates/game/dist"
exec python3 -m http.server "${PORT:-8080}" --bind 0.0.0.0
