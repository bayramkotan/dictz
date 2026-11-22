#!/bin/bash
# Take the README screenshots of the Rust edition under a virtual display.
#
#   docs/screenshots/shoot_rust.sh [OUT_DIR]
#
# Needs Xvfb, xdotool, ImageMagick (import) and a built dictz-rs
# (cargo build --release in dictz-rs). Writes main-dark.png (Dracula),
# rust-light.png and arabic-rust.png at 2720×1640 (1360×820 at a zoom of 200 %).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
OUT=${1:-$HERE}
BIN=${DICTZ_BIN:-$ROOT/dictz-rs/target/release/dictz}
[ -x "$BIN" ] || BIN=$ROOT/dictz-rs/target/debug/dictz
TMP=$(mktemp -d /tmp/dictz-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
QT_QPA_PLATFORM=offscreen python3 "$HERE/demo_dicts.py" "$TMP/Dictionaries" >/dev/null
DSP=:77

shot() { # shot NAME LANGUAGE THEME
    rm -rf "$TMP/data"
    mkdir -p "$TMP/data"
    cat >"$TMP/data/settings.json" <<EOF
{"general": {"language": "$2", "load_in_background": false, "restore_last_search": false},
 "appearance": {"theme": "$3", "ui_scale": 200},
 "center": {"refresh_days": 0},
 "dictionaries": {"folders": ["$TMP/Dictionaries"], "scan_on_startup": true}}
EOF
    Xvfb $DSP -screen 0 2720x1640x24 >/dev/null 2>&1 &
    local xp=$!
    sleep 1
    DISPLAY=$DSP DICTZ_DATA_DIR="$TMP/data" LIBGL_ALWAYS_SOFTWARE=1 \
        "$BIN" gui >"$TMP/gui.log" 2>&1 &
    local ap=$!
    sleep 8
    local w
    w=$(DISPLAY=$DSP xdotool search --name dictz | head -1)
    DISPLAY=$DSP xdotool windowsize "$w" 2720 1640 windowmove "$w" 0 0 windowfocus "$w"
    sleep 2
    DISPLAY=$DSP xdotool type --delay 90 "apple"
    DISPLAY=$DSP xdotool key Return
    sleep 3
    DISPLAY=$DSP import -window root "$OUT/$1"
    echo "wrote $OUT/$1"
    kill $ap $xp 2>/dev/null || true
    wait $ap $xp 2>/dev/null || true
}

shot main-dark.png en dracula
shot rust-light.png en light
shot arabic-rust.png ar light
