#!/usr/bin/env bash
# Bigger clusters (galaxy plan track E0): sim's `cluster` example natively and, with --wasm, also as wasm32-wasip1
# under Node's WASI (V8, the browser's engine), each with the load average before and after; then whether every
# row's state hash matched. Arguments after the flags go to the example (see crates/core/sim/examples/cluster.rs).
#
#   scripts/cluster-bench.sh [--wasm] [--section cost,hitch,precision] [--awake 20,40] [--repeat 5] [...]
#
# Env: JOBS (2), CARGO_TARGET_DIR (target), OUT ($CARGO_TARGET_DIR/cluster-bench).
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env" 2>/dev/null || true
WASM=0
if [ "${1:-}" = "--wasm" ]; then WASM=1; shift; fi
JOBS=${JOBS:-2}
TARGET=${CARGO_TARGET_DIR:-target}
OUT=${OUT:-$TARGET/cluster-bench}
mkdir -p "$OUT"
cargo build -q --release -p sim --example cluster -j "$JOBS"
[ $WASM = 1 ] && cargo build -q --release -p sim --example cluster --target wasm32-wasip1 -j "$JOBS"
run() { # run <label> <command...>
  local label=$1; shift
  echo "## $label (load average before: $(cut -d' ' -f1-3 /proc/loadavg))"
  "$@" | tee "$OUT/$label.txt"
  echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
}
run native "$TARGET/release/examples/cluster" "$@"
if [ $WASM = 1 ]; then
  run wasm node --no-warnings scripts/wasi-run.mjs "$TARGET/wasm32-wasip1/release/examples/cluster.wasm" "$@"
  hashes() { grep -o '| [0-9a-f]\{16\} |$' "$1"; }
  if diff <(hashes "$OUT/native.txt") <(hashes "$OUT/wasm.txt") > /dev/null; then
    echo "hashes: IDENTICAL native vs wasm ($(hashes "$OUT/native.txt" | wc -l) rows)"
  else
    echo "hashes: DIFFER native vs wasm"; exit 1
  fi
fi
