#!/usr/bin/env bash
# Regenerates THIRD_PARTY_NOTICES.md from the Rust dependency tree (ADR-0047, docs/client.md "Licences and
# third-party notices"). Needs cargo-about (`cargo binstall cargo-about`). Run it after adding, removing or upgrading
# a dependency and commit the result with the change. Exits non-zero when a crate's licence isn't in
# scripts/notices/about.toml's `accepted` list or can't be read.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo about generate --workspace --locked --fail \
    -c scripts/notices/about.toml \
    -o THIRD_PARTY_NOTICES.md \
    scripts/notices/notices.md.hbs
echo "THIRD_PARTY_NOTICES.md: $(grep -c '^- \[' THIRD_PARTY_NOTICES.md) entries"
