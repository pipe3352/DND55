#!/usr/bin/env bash
# Real-input regression test for mouse-wheel routing (Linux, Xvfb + xdotool + ImageMagick).
# A DM host opens the map editor; then real wheel events are sent:
#   1. wheel over the asset library (left panel) ⇒ the panel scrolls, the map zoom does NOT change
#   2. wheel over the map canvas                 ⇒ the map zoom changes
# Zoom changes are read from the app's `CAMERA-ZOOM:` log lines (printed in DND55_AUTO mode).
# Usage: scripts/gui-wheel-test.sh [path/to/DND55]
set -euo pipefail
BIN=${1:-target/debug/DND55}
OUT=${OUT:-target/gui-wheel-test}
rm -rf "$OUT"; mkdir -p "$OUT"
Xvfb :93 -screen 0 1600x900x24 >/dev/null 2>&1 & X1=$!
trap 'kill $X1 2>/dev/null || true' EXIT
sleep 1
export DISPLAY=:93 DND55_SESSION_CODE=WHEEL1 DND55_NO_GPU_FALLBACK=1
DND55_DATA_DIR="$OUT/dm" DND55_AUTO=host DND55_AUTO_SCENARIO=calm DND55_AUTO_PORT=7631 \
  DND55_AUTO_OPEN=select DND55_AUTO_OPEN_AT=6 DND55_SCREENSHOT="$OUT/final.png" DND55_SCREENSHOT_AFTER=34 \
  "$BIN" > "$OUT/dm.log" 2>&1 &
APP=$!
sleep 16
zooms() { grep -c "CAMERA-ZOOM" "$OUT/dm.log" || true; }
shot() { import -window root "$OUT/$1.png"; }
wheel() { for _ in $(seq "$3"); do xdotool click "$4"; sleep 0.08; done; }
# 1) Over the asset library.
xdotool mousemove 190 640; sleep 1
shot before-panel
z0=$(zooms)
wheel 190 640 8 5
sleep 1.5
shot after-panel
z1=$(zooms)
# 2) Over the map canvas.
xdotool mousemove 900 470; sleep 1
wheel 900 470 3 4
sleep 1.5
z2=$(zooms)
shot after-map
wait $APP || true
fail=0
# The panel area must have changed (the list scrolled).
panel_diff=$(compare -metric AE -crop 330x600+20+200 "$OUT/before-panel.png" "$OUT/after-panel.png" null: 2>&1 || true)
if [ "${panel_diff%% *}" != "0" ]; then echo "PASS: wheel over the asset library scrolled it ($panel_diff px changed)"; else echo "FAIL: asset library did not scroll"; fail=1; fi
if [ "$z1" = "$z0" ]; then echo "PASS: wheel over the asset library left the map zoom unchanged"; else echo "FAIL: map zoomed while scrolling the asset library ($z0 -> $z1 zoom changes)"; fail=1; fi
if [ "$z2" -gt "$z1" ]; then echo "PASS: wheel over the map zoomed it ($((z2 - z1)) zoom changes)"; else echo "FAIL: wheel over the map did not zoom"; fail=1; fi
grep "MAP-POINTER" "$OUT/dm.log" | tail -4 || true
echo "Screenshots: $OUT/before-panel.png $OUT/after-panel.png $OUT/after-map.png"
exit $fail
