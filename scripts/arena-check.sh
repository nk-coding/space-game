#!/usr/bin/env bash
# The balance check (GDR-0032 "a short run checks every data change", coop-arena track A3; docs/arena.md "The balance
# check"): builds the `arena` CLI (release) and runs `arena check`: the fight set crates/core/arena/check/set.ron,
# each fight on its fixed seeds, against the stored baseline crates/core/arena/check/baseline.ron. Prints a row per
# fight (wins, side 0's mean score, mean time, whether the fights changed at all) and exits 1 when a fight leaves its
# tolerance. Run it after every content or sim change; when the shift is intended, write the baseline anew and commit
# it with the change.
#
#   scripts/arena-check.sh             # check
#   scripts/arena-check.sh --update    # run and rewrite the baseline
#   scripts/arena-check.sh [<set.ron>] [--baseline FILE] [--update] [--threads N]
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q -p arena-cli -j "${CARGO_BUILD_JOBS:-6}"
exec target/release/arena check "$@"
