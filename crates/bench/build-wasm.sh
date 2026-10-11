#!/usr/bin/env bash
# Builds one wasm binary per engine (rapier, box2d-rust, avian) into web/pkg/<engine>/ via wasm-bindgen
# (--target web; the same files are used by the Node runner and the browser page) and records sizes.
set -euo pipefail
cd "$(dirname "$0")"
source "$HOME/.cargo/env" 2>/dev/null || true
JOBS=${JOBS:-6}

# wasm-bindgen-cli must match the wasm-bindgen version in Cargo.lock exactly.
WB=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/,""); print; exit}' Cargo.lock)
if ! command -v wasm-bindgen >/dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WB" ]; then
  cargo binstall -y "wasm-bindgen-cli@$WB"
fi

: > web/pkg-sizes.txt.tmp
for eng in rapier box2d avian; do
  echo "== building wasm for $eng"
  cargo build -p bench-wasm --target wasm32-unknown-unknown --profile wasm-release \
    --no-default-features --features "$eng" -j "$JOBS"
  raw=target/wasm32-unknown-unknown/wasm-release/bench_wasm.wasm
  mkdir -p "web/pkg/$eng"
  wasm-bindgen --target web --remove-name-section --remove-producers-section --out-dir "web/pkg/$eng" --out-name bench "$raw"
  wasm="web/pkg/$eng/bench_bg.wasm"
  size=$(stat -c %s "$wasm")
  gz=$(gzip -9 -c "$wasm" | wc -c)
  echo "$eng $size $gz" >> web/pkg-sizes.txt.tmp
done
mv web/pkg-sizes.txt.tmp web/pkg/sizes.txt
echo "wasm sizes (bytes: raw, gzip -9):"
cat web/pkg/sizes.txt
