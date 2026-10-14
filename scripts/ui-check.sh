#!/usr/bin/env bash
# Screenshots of the web client at three window sizes, one after the other (Niklas, 14 Oct 2026: layout bugs were
# common, and checks at one size missed every bug of the UI scale): 1280x720 and 1600x900 run the UI at scale 1,
# 2560x1440 scales it up about 1.8x like Niklas's screen, where fractional pixels round unevenly.
#
# Usage: scripts/ui-check.sh <url> <out-dir> "<steps>"
# The steps are web-drive.mjs steps (see there); each size gets its own folder <out-dir>/<w>x<h>/. Waits run 2.5x as
# long at 2560x1440 (SwiftShader draws it far slower). Use keys rather than clicks where you can: coordinates differ
# per size. For a close-up, run web-drive.mjs at one size with a `crop` step and enlarge it with scripts/png-zoom.py.
# Look at every PNG (the Read tool). One browser at a time: on a small VM nothing else should build meanwhile.
set -euo pipefail
url="${1:?usage: ui-check.sh <url> <out-dir> \"<steps>\"}"
out="${2:?usage: ui-check.sh <url> <out-dir> \"<steps>\"}"
steps="${3:-}"
here="$(dirname "$0")"
for size in 1280x720 1600x900 2560x1440; do
  w="${size%x*}" h="${size#*x}"
  scaled="$steps"
  if [[ "$size" == 2560x1440 ]]; then
    scaled="$(perl -pe 's/\bwait (\d+)/"wait " . int($1 * 2.5)/ge' <<<"$steps")"
  fi
  echo "== $size"
  timeout 600 node "$here/web-drive.mjs" "$url" "$out/$size" "$scaled" "$w" "$h" --quiet-console | grep -E '^\[(shot|crop|pageerror)\]|panicked| ERROR ' || true
done
