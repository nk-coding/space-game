#!/usr/bin/env bash
# Cross-platform determinism check: runs a scenario natively and as WebAssembly (wasm32-wasip1 under Node's
# WASI) and compares the per-tick state hashes. Reports the first divergent tick; exit 1 on divergence.
#
#   scripts/determinism-check.sh [--seed N] [--ticks N] [--scenario NAME] [--package CRATE] [--example NAME]
#   scripts/determinism-check.sh --fight NAME [--seed N] [--ticks N]   # an arena fight (crates/core/arena/fights/NAME.ron)
#   scripts/determinism-check.sh --fight SGF1:…            # a fight code (GDR-0092: `arena fight|evolve` print them)
#   scripts/determinism-check.sh --compare A.txt B.txt     # compare two existing hash streams
#
# Scenarios of sim's `determinism` example (sim::voyage::scenario): core (default), flight, assists (flight with pilot
# assists), load (--ships N in the example), behavior, salvage (wrecks spawned as wrecks, collecting, fragments, a lost
# ship's cargo), voyage (a voyage directed by sim::voyage: sites spawning, sleeping, waking and clearing, salvage, the
# recall check).
# `--fight NAME` runs arena's `fight_hashes` example instead: the fight spec crates/core/arena/fights/NAME.ron
# (runner_vs_raiders, skiff_mirror) with the seed replaced, until the fight ends or --ticks have run (default 2700:
# the 90 s time limit). A fight code (`--fight SGF1:…`) runs its own spec and seed to its end and says whether it
# ends as recorded (`# record=as recorded`); its files are named after a short hash of the code.
#
# Hash stream format (what the example prints): one "<tick> <hash>" line per tick; lines starting with '#'
# are comments (header, summary) and are shown but not compared. Any producer that prints this format can
# be checked: the physics layer adds scenarios to sim's `determinism` example (--scenario), and the browser
# build can print the same lines to the console (captured with tools/headless/shot.mjs) for --compare.
#
# Env: JOBS (6), PROFILE (release; or dev), CARGO_TARGET_DIR (target), OUT ($CARGO_TARGET_DIR/determinism).
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env" 2>/dev/null || true

compare() { # compare <label-a> <file-a> <label-b> <file-b>
  local la=$1 a=$2 lb=$3 b=$4
  grep '^#' "$a" | sed "s/^/[$la] /" || true
  grep '^#' "$b" | sed "s/^/[$lb] /" || true
  awk -v la="$la" -v lb="$lb" '
    NR == FNR { if ($0 !~ /^#/ && NF >= 2) { A[++na] = $0 } ; next }
    $0 !~ /^#/ && NF >= 2 { B[++nb] = $0 }
    END {
      n = (na < nb) ? na : nb
      for (i = 1; i <= n; i++) {
        if (A[i] != B[i]) {
          split(A[i], x, " "); split(B[i], y, " ")
          printf "DIVERGED at tick %s: %s=%s %s=%s\n", x[1], la, x[2], lb, y[2]
          exit 1
        }
      }
      if (na != nb) { printf "LENGTH MISMATCH: %s has %d ticks, %s has %d\n", la, na, lb, nb; exit 1 }
      if (n == 0) { print "NO TICKS to compare"; exit 1 }
      printf "IDENTICAL: %d ticks, last tick %s\n", n, A[n]
    }' "$a" "$b"
}

if [ "${1:-}" = "--compare" ]; then
  [ $# -eq 3 ] || { echo "usage: $0 --compare A.txt B.txt" >&2; exit 2; }
  compare A "$2" B "$3"
  exit $?
fi

SEED=42; TICKS=; SCENARIO=core; PACKAGE=sim; EXAMPLE=determinism; DEFAULT_TICKS=900
while [ $# -gt 0 ]; do
  case "$1" in
    --seed) SEED=$2; shift 2 ;;
    --ticks) TICKS=$2; shift 2 ;;
    --scenario) SCENARIO=$2; shift 2 ;;
    --package) PACKAGE=$2; shift 2 ;;
    --example) EXAMPLE=$2; shift 2 ;;
    --fight) PACKAGE=arena; EXAMPLE=fight_hashes; SCENARIO=$2; DEFAULT_TICKS=2700; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
# A fight code carries its seed and runs to its end (at most the arena's 30-minute bound).
CODE=0
case "$SCENARIO" in SGF*) CODE=1; DEFAULT_TICKS=54000 ;; esac
TICKS=${TICKS:-$DEFAULT_TICKS}
JOBS=${JOBS:-6}
PROFILE=${PROFILE:-release}
TARGET=${CARGO_TARGET_DIR:-target}
OUT=${OUT:-$TARGET/determinism}
DIR=$PROFILE; [ "$PROFILE" = dev ] && DIR=debug

if ! rustup target list --installed | grep -qx wasm32-wasip1; then
  rustup target add wasm32-wasip1
fi
cargo build -q -p "$PACKAGE" --example "$EXAMPLE" --profile "$PROFILE" -j "$JOBS"
cargo build -q -p "$PACKAGE" --example "$EXAMPLE" --profile "$PROFILE" --target wasm32-wasip1 -j "$JOBS"

mkdir -p "$OUT"
ARGS=(--seed "$SEED" --ticks "$TICKS" --scenario "$SCENARIO")
LABEL="$SCENARIO-$SEED"
if [ $CODE = 1 ]; then
  ARGS=(--ticks "$TICKS" --scenario "$SCENARIO")
  LABEL="code-$(printf %s "$SCENARIO" | md5sum | cut -c1-12)"
fi
NATIVE_OUT="$OUT/$EXAMPLE-$LABEL-native.txt"
WASM_OUT="$OUT/$EXAMPLE-$LABEL-wasm.txt"
"$TARGET/$DIR/examples/$EXAMPLE" "${ARGS[@]}" > "$NATIVE_OUT"
node --no-warnings scripts/wasi-run.mjs "$TARGET/wasm32-wasip1/$DIR/examples/$EXAMPLE.wasm" "${ARGS[@]}" > "$WASM_OUT"
echo "native ($(uname -m)) vs wasm32-wasip1 (node $(node --version)), profile $PROFILE"
compare native "$NATIVE_OUT" wasm "$WASM_OUT"
