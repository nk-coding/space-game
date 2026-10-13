#!/usr/bin/env bash
# Optimised web build into crates/game/dist, then report sizes. Serve it with `scripts/web-serve-dist.sh`.
# The web build is the development platform (not shipped to players), so it is tuned for speed: cargo profile
# `release` (opt-level 3) + wasm-opt -O3 (`WASM_PROFILE=<profile>` picks another cargo profile).
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$(dirname "$0")/../crates/game"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
trunk build --release --cargo-profile "${WASM_PROFILE:-release}" "$@"
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
