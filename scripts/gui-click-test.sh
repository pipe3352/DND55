#!/usr/bin/env bash
# Real-input GUI test (Linux, Xvfb + xdotool): two game windows (DM host + player) on virtual
# displays, driven by actual mouse events. Asserts on the adventure log each client received.
#
#   1. Player (Wizard): Spellbook → "Cast" Fire Bolt → click the goblin  ⇒ "casts Fire Bolt"
#   2. DM: drag "Aboleth" from the Bestiary list onto the map          ⇒ "Aboleth 1" is on the map
#
# Coordinates assume a 1600×900 window at UI scale 1.0.
# Usage: scripts/gui-click-test.sh [path/to/DND55]
set -euo pipefail
BIN=${1:-target/debug/DND55}
OUT=${OUT:-target/gui-test}
rm -rf "$OUT"; mkdir -p "$OUT"
Xvfb :97 -screen 0 1600x900x24 >/dev/null 2>&1 & X1=$!
Xvfb :98 -screen 0 1600x900x24 >/dev/null 2>&1 & X2=$!
trap 'kill $X1 $X2 2>/dev/null || true' EXIT
sleep 1
export DND55_SESSION_CODE=TEST42 DND55_NO_GPU_FALLBACK=1
PORT=7611
DISPLAY=:97 DND55_DATA_DIR="$OUT/dm" DND55_AUTO=host DND55_AUTO_SCENARIO=calm DND55_AUTO_PORT=$PORT \
  DND55_AUTO_OPEN=bestiary DND55_AUTO_OPEN_AT=12 DND55_SCREENSHOT="$OUT/dm.png" DND55_SCREENSHOT_AFTER=26 \
  "$BIN" > "$OUT/dm.log" 2>&1 &
DM=$!
sleep 5
DISPLAY=:98 DND55_DATA_DIR="$OUT/player" DND55_AUTO=join DND55_AUTO_ADDR=127.0.0.1:$PORT DND55_AUTO_CLASS=wizard \
  DND55_AUTO_OPEN=spellbook DND55_AUTO_OPEN_AT=12 DND55_SCREENSHOT="$OUT/player.png" DND55_SCREENSHOT_AFTER=22 \
  "$BIN" > "$OUT/player.log" 2>&1 &
PL=$!
sleep 12
click() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool mousedown 1; sleep 0.15; xdotool mouseup 1; }
# Player: cast Fire Bolt at the goblin.
DISPLAY=:98 click 496 228
sleep 1
DISPLAY=:98 click 623 340
# DM: drag a monster from the bestiary list onto the map.
export DISPLAY=:97
# Press on the centre of the first row ("Aboleth"; the window is pinned at 18,18), start the drag
# inside that row (egui's drag threshold), then carry it over the map and drop.
xdotool mousemove 200 222; sleep 0.3; xdotool mousedown 1; sleep 0.2
for x in 206 214 224; do xdotool mousemove $x 222; sleep 0.1; done
for x in 400 600 800 900; do xdotool mousemove $x 250; sleep 0.1; done
sleep 0.3; xdotool mouseup 1
unset DISPLAY
wait $PL $DM
fail=0
grep -q "GAMELOG: .*casts Fire Bolt" "$OUT/player.log" && echo "PASS: player cast Fire Bolt via mouse" || { echo "FAIL: no Fire Bolt in player log"; fail=1; }
grep -q "GAMELOG-CREATURES: .*Aboleth 1" "$OUT/dm.log" && echo "PASS: DM dragged a monster from the bestiary onto the map ($(grep GAMELOG-CREATURES "$OUT/dm.log"))" || { echo "FAIL: no dragged monster on the DM map"; fail=1; }
echo "Screenshots: $OUT/dm.png $OUT/player.png"
exit $fail
