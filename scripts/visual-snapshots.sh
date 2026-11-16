#!/usr/bin/env bash
# Captures screenshots of the running app for visual review (Linux, Xvfb, software Vulkan).
#   scripts/visual-snapshots.sh <out-dir> [binary]
# Produces: menu.png, showcase-dm.png, showcase-player.png, graph.png, editor.png, sheet.png, creator.png
set -uo pipefail
OUT=${1:-target/snapshots}; BIN=${2:-target/debug/DND55}
mkdir -p "$OUT"
Xvfb :95 -screen 0 1600x900x24 >/dev/null 2>&1 & X1=$!
Xvfb :96 -screen 0 1600x900x24 >/dev/null 2>&1 & X2=$!
trap 'kill $X1 $X2 2>/dev/null || true' EXIT
sleep 1
export DND55_SESSION_CODE=SNAP42 DND55_NO_GPU_FALLBACK=1
run() { # name display port after env...
  local name=$1 disp=$2 after=$3; shift 3
  rm -rf "$OUT/data-$name"
  env DISPLAY=$disp DND55_DATA_DIR="$OUT/data-$name" DND55_SCREENSHOT="$OUT/$name.png" DND55_SCREENSHOT_AFTER=$after "$@" "$BIN" > "$OUT/$name.log" 2>&1
}
run menu :95 6
for pass in "showcase-dm|showcase-player||wizard" "graph|sheet|graph|" "editor|creator|select|creator"; do
  IFS='|' read -r dm pl dmopen plopen <<<"$pass"
  P=$((7700 + RANDOM % 200))
  run "$dm" :95 18 DND55_AUTO=host DND55_AUTO_SCENARIO=showcase DND55_AUTO_PORT=$P DND55_AUTO_OPEN="$dmopen" DND55_AUTO_OPEN_AT=12 &
  DMPID=$!
  sleep 4
  run "$pl" :96 16 DND55_AUTO=join DND55_AUTO_ADDR=127.0.0.1:$P DND55_AUTO_CLASS=wizard DND55_AUTO_OPEN="$plopen" DND55_AUTO_OPEN_AT=12 &
  PLPID=$!
  wait $DMPID $PLPID
done
ls -la "$OUT"/*.png
