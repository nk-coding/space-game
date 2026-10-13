#!/usr/bin/env bash
# Web build into crates/game/dist, then report sizes. Serve it with `scripts/web-serve-dist.sh`.
# The web build is the development platform (not shipped to players). By default it is the everyday build (ADR-0034):
# cargo profile `web-dev` (opt-level 3, no LTO, 16 codegen units, incremental) and no wasm-opt, about a minute
# after a change in `game`. `--full` is the optimised build for performance checks: profile `release` (thin LTO,
# one codegen unit) + wasm-opt -O3, about five minutes (`WASM_PROFILE=<profile>` picks another cargo profile for it).
# Other arguments go to trunk.
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$(dirname "$0")/../crates/game"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
if [[ "${1:-}" == "--full" ]]; then
  shift
  trunk build --release --cargo-profile "${WASM_PROFILE:-release}" "$@"
else
  # Without --release trunk runs no wasm-opt (index.html's data-wasm-opt applies to release builds only).
  trunk build --cargo-profile web-dev "$@"
fi
wasm=$(ls dist/*.wasm | head -1)
js=$(ls dist/*.js | head -1)
raw=$(stat -c %s "$wasm")
gz=$(gzip -9 -c "$wasm" | wc -c)
if command -v brotli >/dev/null; then
  br=$(brotli -q 11 -c "$wasm" | wc -c)
else # node's zlib has brotli
  br=$(node -e 'const z=require("zlib");process.stdout.write(String(z.brotliCompressSync(require("fs").readFileSync(process.argv[1]),{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length))' "$wasm")
fi
mib() { echo "scale=2; $1/1048576" | bc; }
printf 'wasm: %s\n  raw    %9d bytes (%s MiB)\n  gzip   %9d bytes (%s MiB)\n  brotli %9d bytes (%s MiB)\njs:   %d bytes\n' \
  "$wasm" "$raw" "$(mib "$raw")" "$gz" "$(mib "$gz")" "$br" "$(mib "$br")" "$(stat -c %s "$js")"
