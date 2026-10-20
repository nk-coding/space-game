#!/usr/bin/env bash
# Serves the last build in crates/client/game/dist without rebuilding (e.g. after scripts/web-build.sh), on PORT
# (default 8081).
#   --base /space-game/  serves it under that path, as GitHub Pages does (http://10.77.9.2:8081/space-game/); build
#                        it for that path first: scripts/web-build.sh --full --public-url /space-game/
#   --wiki               with --base: also builds the wiki for <base>docs/ (WIKI_BASE) and serves it there
#   --dist <dir>         serves another trunk dist directory
# Python's server sends no gzip (GitHub Pages does), so the download is the raw size.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dist="$root/crates/client/game/dist"
base=""
wiki=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --base) base="$2"; shift 2 ;;
    --wiki) wiki=1; shift ;;
    --dist) dist="$(cd "$2" && pwd)"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
port="${PORT:-8081}"
if [[ -z "$base" || "$base" == "/" ]]; then
  [[ $wiki == 0 ]] || { echo "--wiki needs --base" >&2; exit 2; }
  cd "$dist"
  exec python3 -m http.server "$port" --bind 0.0.0.0
fi
base="/${base#/}"; base="${base%/}/" # /space-game/
# A tree of links: <tree>/<base>/<each file of dist> and <base>/docs -> the wiki's build. Nothing is copied.
tree="$root/target/serve-base-$port"
rm -rf "$tree"
mkdir -p "$tree$base"
for f in "$dist"/*; do ln -s "$f" "$tree$base$(basename "$f")"; done
if [[ $wiki == 1 ]]; then
  (cd "$root/wiki" && WIKI_BASE="${base}docs/" npm run build)
  rm -f "$tree${base}docs"
  ln -s "$root/wiki/.vitepress/dist" "$tree${base}docs"
fi
echo "serving $dist at http://0.0.0.0:$port$base"
cd "$tree"
exec python3 -m http.server "$port" --bind 0.0.0.0
