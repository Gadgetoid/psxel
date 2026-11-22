#!/bin/sh
set -e
FX_SOUNDS="${FX_SOUNDS:-$HOME/Development/fx/sounds/build/fx-sounds}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$HERE/../.."
WORK="$(mktemp -d)"
"$FX_SOUNDS" -o "$WORK" "$HERE/rally.ini"
"$FX_SOUNDS" -o "$WORK" -s amb_rain "$(dirname "$FX_SOUNDS")/../presets/ambience.ini"
mkdir -p "$ROOT/assets/sounds"
for file in "$WORK"/*_dry.wav; do
    name="$(basename "$file" _dry.wav)"
    cp "$file" "$ROOT/assets/sounds/$name.wav"
done
rm -rf "$WORK"
