#!/usr/bin/env bash
# Native load test: the shared load scenario (sim::voyage::scenario::load_test, also used by the browser client) with
# n = 1, 5 and 20 ships for 1800 ticks each; prints a markdown table of step times and counts plus the load
# average before and after (VM timings are noisy: repeat and report medians).
#
#   scripts/load-test-native.sh [--ships 1,5,20] [--ticks 1800] [--seed 42]
#
# Env: JOBS (6), CARGO_TARGET_DIR (target).
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env" 2>/dev/null || true
JOBS=${JOBS:-6}
TARGET=${CARGO_TARGET_DIR:-target}
cargo build -q --release -p sim --example load_test -j "$JOBS"
echo "load average before: $(cut -d' ' -f1-3 /proc/loadavg)"
"$TARGET/release/examples/load_test" "$@"
echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
