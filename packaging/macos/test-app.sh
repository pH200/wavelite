#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
"$root/packaging/macos/build-app.sh"

executable="$root/target/macos/WaveLite.app/Contents/MacOS/WaveLite"
screenshots=(
    "$root/target/screenshots/macos-native/wave3-mk1.png"
    "$root/target/screenshots/macos-native/wave3-mk2.png"
    "$root/target/screenshots/macos-native/wave3-mk2-dynamics.png"
    "$root/target/screenshots/macos-native/wave3-mk2-eq.png"
)
rm -f "${screenshots[@]}"

WAVELITE_FAKE_MODEL=wave3-mk1 \
WAVELITE_FAKE_DELAY_MS=150 \
WAVELITE_DEMO_SELF_TEST=1 \
WAVELITE_SCREENSHOT="${screenshots[0]}" \
    "$executable"

WAVELITE_FAKE_MODEL=wave3-mk2 \
WAVELITE_FAKE_DELAY_MS=150 \
WAVELITE_DEMO_SELF_TEST=1 \
WAVELITE_SCREENSHOT="${screenshots[1]}" \
    "$executable"

WAVELITE_FAKE_MODEL=wave3-mk2 \
WAVELITE_FAKE_WAVE_FX=dynamics \
WAVELITE_SCREENSHOT="${screenshots[2]}" \
    "$executable"

WAVELITE_FAKE_MODEL=wave3-mk2 \
WAVELITE_FAKE_WAVE_FX=eq \
WAVELITE_SCREENSHOT="${screenshots[3]}" \
    "$executable"

for screenshot in "${screenshots[@]}"; do
    test -s "$screenshot"
    echo "Verified $screenshot"
done
