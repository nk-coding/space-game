#!/usr/bin/env bash
# Web build into crates/client/game/dist, then report its size (compressed sizes only with --full). Serve it with `scripts/web-serve-dist.sh`.
# The web build is a player platform too (ADR-0048; `--full` is what GitHub Pages gets). By default it is the everyday build (ADR-0034):
# cargo profile `web-dev` (opt-level 3, no LTO, 16 codegen units, incremental) and no wasm-opt, about 30 s
# after a change in `game`. `--full` is the published build: profile `web-release` (the client compiled for size, the
# sim for speed, fat LTO; ADR-0049) + wasm-opt -O3, about ten minutes cold (`WASM_PROFILE=<profile>` picks another
# cargo profile for it, e.g. `release` for the all-speed build).
# Other arguments go to trunk, e.g. `--public-url /space-game/` (the path GitHub Pages serves it under; serve it there
# with `scripts/web-serve-dist.sh --base /space-game/`) or `--dist <dir>` (another output directory).
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$(dirname "$0")/../crates/client/game"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
full=0
dist=dist
prev=""
for a in "$@"; do
  [[ "$prev" == "--dist" ]] && dist="$a"
  [[ "$a" == --dist=* ]] && dist="${a#--dist=}"
  prev="$a"
done
if [[ "${1:-}" == "--full" ]]; then
  shift
  full=1
  trunk build --release --cargo-profile "${WASM_PROFILE:-web-release}" "$@"
else
  # Without --release trunk runs no wasm-opt (index.html's data-wasm-opt applies to release builds only).
  trunk build --cargo-profile web-dev "$@"
fi
wasm=$(ls "$dist"/game-*_bg.wasm | head -1)
js=$(ls "$dist"/game-*.js | head -1)
mib() { echo "scale=2; $1/1048576" | bc; }
raw=$(stat -c %s "$wasm")
if [[ $full == 0 ]]; then
  # The everyday build only says its size: compressing for the report (brotli -q 11) took longer than the build.
  printf 'wasm: %s\n  raw    %9d bytes (%s MiB)\njs:   %d bytes\n' "$wasm" "$raw" "$(mib "$raw")" "$(stat -c %s "$js")"
  exit 0
fi
gz=$(gzip -9 -c "$wasm" | wc -c)
if command -v brotli >/dev/null; then
  br=$(brotli -q 11 -c "$wasm" | wc -c)
else # node's zlib has brotli
  br=$(node -e 'const z=require("zlib");process.stdout.write(String(z.brotliCompressSync(require("fs").readFileSync(process.argv[1]),{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length))' "$wasm")
fi
printf 'wasm: %s\n  raw    %9d bytes (%s MiB)\n  gzip   %9d bytes (%s MiB)\n  brotli %9d bytes (%s MiB)\njs:   %d bytes\n' \
  "$wasm" "$raw" "$(mib "$raw")" "$gz" "$(mib "$gz")" "$br" "$(mib "$br")" "$(stat -c %s "$js")"
