#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
library=${1:-$root/target/debug/libwavelite.a}
bindgen=${UNIFFI_BINDGEN:-uniffi-bindgen}
generated="$root/target/macos/uniffi-generated"

if ! command -v "$bindgen" >/dev/null 2>&1; then
    echo "error: uniffi-bindgen 0.32.2 not found; install it with 'cargo install uniffi --version 0.32.2 --features cli --locked' or set UNIFFI_BINDGEN" >&2
    exit 1
fi
if [[ ! -s "$library" ]]; then
    echo "error: Rust static library not found: $library; run 'cargo build --locked --no-default-features' first" >&2
    exit 1
fi

rm -rf "$generated"
mkdir -p "$generated"
"$bindgen" generate \
    --library "$library" \
    --crate wavelite \
    --language swift \
    --out-dir "$generated"

mkdir -p \
    "$root/macos/Sources/WaveLiteKit" \
    "$root/macos/Sources/WaveLiteCoreFFI/include"
install -m 644 "$generated/WaveLiteKit.swift" \
    "$root/macos/Sources/WaveLiteKit/WaveLiteKit.swift"
install -m 644 "$generated/WaveLiteCoreFFI.h" \
    "$root/macos/Sources/WaveLiteCoreFFI/include/WaveLiteCoreFFI.h"
install -m 644 "$generated/WaveLiteCoreFFI.modulemap" \
    "$root/macos/Sources/WaveLiteCoreFFI/include/module.modulemap"
