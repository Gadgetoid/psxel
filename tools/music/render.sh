#!/bin/sh
set -e
HORN="${HORN_SYNTHWAVE:-$HOME/Development/fx/horn-synthwave}"
HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
python3 "$HERE/compose_endless.py" > "$WORK/rally_music.ini"
"$HORN/build/fx-sounds" -o "$WORK" "$WORK/rally_music.ini"
cargo run --release -q -p psx_spu --bin spu-encode -- "$WORK/rally_music_wet.wav" "$HERE/../../assets/sounds/rally_music.spu"
rm -rf "$WORK"
