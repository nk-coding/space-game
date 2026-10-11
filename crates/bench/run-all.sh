#!/usr/bin/env bash
# Re-runs every native + Node measurement and writes raw results to results/<stamp>/:
#   env.txt            machine, toolchain, load average before/after
#   native-st.jsonl    native, 1 thread   (each repeat is a separate process -> also checks cross-process determinism)
#   native-mt.jsonl    native, engine multithreading with $MT_THREADS threads (engines that support it)
#   node.jsonl         wasm in Node (V8), 1 thread
#   *.log              human-readable per-run lines
#   summary.md         tables (node/report.mjs)
# Environment knobs: REPEATS (3) TICKS (900) WARMUP (30) MT_THREADS (4) JOBS (6) SKIP_BUILD (0)
#                    ENGINES / SCENARIOS (comma lists, default all) SKIP_MT / SKIP_NODE (0)
set -euo pipefail
cd "$(dirname "$0")"
source "$HOME/.cargo/env" 2>/dev/null || true
REPEATS=${REPEATS:-3}; TICKS=${TICKS:-900}; WARMUP=${WARMUP:-30}; MT_THREADS=${MT_THREADS:-4}; JOBS=${JOBS:-6}
STAMP=${STAMP:-$(date +%Y%m%d-%H%M)}
OUT=results/$STAMP
mkdir -p "$OUT" bin
ENGINE_LIST=${ENGINES:-rapier,rapier-compound,box2d-rust,avian,avian-compound,box2d-c}
SCEN_LIST=${SCENARIOS:-ships,ships-merged,debris,raycasts,breakage,breakage-merged,asteroids,combined,combined-merged}
MT_SCEN_LIST=${MT_SCENARIOS:-$SCEN_LIST}
# Engine id -> cargo feature / binary suffix (one engine per binary, same feature set as the wasm build).
build_of() { case $1 in rapier*) echo rapier;; box2d-rust) echo box2d;; avian*) echo avian;; box2d-c) echo boxdd;; esac; }

if [ "${SKIP_BUILD:-0}" != 1 ]; then
  for b in rapier box2d avian boxdd; do
    cargo build --release -p bench-native --no-default-features --features "$b" -j "$JOBS"
    cp target/release/bench-native "bin/bench-st-$b"
  done
  for b in rapier avian; do   # engines with a multithreading feature (box2d-c threads via workerCount, no feature)
    cargo build --release -p bench-native --no-default-features --features "$b,parallel" -j "$JOBS"
    cp target/release/bench-native "bin/bench-mt-$b"
  done
  cp bin/bench-st-boxdd bin/bench-mt-boxdd
  ./build-wasm.sh
fi

{
  echo "date: $(date -Iseconds)"; echo "host: $(uname -srm)"
  echo "cpu: $(lscpu | sed -n 's/^Model name: *//p') ($(nproc) cores visible)"
  echo "mem: $(free -g | awk '/Mem:/{print $2" GB"}')"
  echo "rustc: $(rustc --version)"; echo "node: $(node --version)"
  echo "load before: $(cut -d' ' -f1-3 /proc/loadavg)"
  echo "settings: repeats=$REPEATS ticks=$TICKS warmup=$WARMUP mt_threads=$MT_THREADS"
  [ -f web/pkg/sizes.txt ] && { echo "wasm sizes (raw, gzip -9 bytes):"; cat web/pkg/sizes.txt; }
} > "$OUT/env.txt"
cat "$OUT/env.txt"

IFS=, read -ra ENGS <<< "$ENGINE_LIST"
IFS=, read -ra SCNS <<< "$SCEN_LIST"
IFS=, read -ra MT_SCNS <<< "$MT_SCEN_LIST"
# Interleaved: repeat -> scenario -> engine, every run in its own process (also checks cross-process determinism).
for rep in $(seq 0 $((REPEATS - 1))); do
  for s in "${SCNS[@]}"; do
    for e in "${ENGS[@]}"; do
      "bin/bench-st-$(build_of "$e")" --engines "$e" --scenarios "$s" --repeats 1 --rep-base "$rep" \
        --warmup "$WARMUP" --ticks "$TICKS" --out "$OUT/native-st.jsonl" | tee -a "$OUT/native-st.log"
    done
  done
done
if [ "${SKIP_MT:-0}" != 1 ]; then
  for rep in $(seq 0 $((REPEATS - 1))); do
    for s in "${MT_SCNS[@]}"; do
      for e in "${ENGS[@]}"; do
        [ "$e" = box2d-rust ] && continue   # serial by design
        "bin/bench-mt-$(build_of "$e")" --engines "$e" --scenarios "$s" --repeats 1 --rep-base "$rep" --threads "$MT_THREADS" \
          --label mt --warmup "$WARMUP" --ticks "$TICKS" --out "$OUT/native-mt.jsonl" | tee -a "$OUT/native-mt.log"
      done
    done
  done
fi
if [ "${SKIP_NODE:-0}" != 1 ]; then
  NODE_ENGS=$(echo "$ENGINE_LIST" | tr ',' '\n' | grep -v '^box2d-c$' | paste -sd,)
  node node/run-node.mjs --repeats "$REPEATS" --warmup "$WARMUP" --ticks "$TICKS" --engines "$NODE_ENGS" \
    --scenarios "$SCEN_LIST" --out "$OUT/node.jsonl" | tee -a "$OUT/node.log"
fi
echo "load after: $(cut -d' ' -f1-3 /proc/loadavg)" >> "$OUT/env.txt"
node node/report.mjs "$OUT"/*.jsonl > "$OUT/summary.md"
echo "== summary: $OUT/summary.md"
cat "$OUT/summary.md"
