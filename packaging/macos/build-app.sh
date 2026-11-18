#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
targets=(x86_64-apple-darwin aarch64-apple-darwin)

cargo=$(command -v cargo 2>/dev/null || true)
if [[ -z "$cargo" ]] && command -v rustup >/dev/null 2>&1; then
    cargo=$(rustup which cargo 2>/dev/null || true)
fi
if [[ -z "$cargo" ]]; then
    echo "error: cargo not found; install a Rust toolchain with 'rustup toolchain install stable'" >&2
    exit 1
fi
for command in swift lipo plutil; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "error: $command not found; install Xcode and Apple's Command Line Tools" >&2
        exit 1
    fi
done
if command -v rustup >/dev/null 2>&1; then
    rustup target add "${targets[@]}"
fi

PATH="$(dirname -- "$cargo"):$PATH"
export PATH
export MACOSX_DEPLOYMENT_TARGET=12.0
for target in "${targets[@]}"; do
    "$cargo" build \
        --manifest-path "$root/Cargo.toml" \
        --target "$target" \
        --release \
        --locked \
        --no-default-features
done

universal="$root/target/macos/universal"
mkdir -p "$universal"
lipo -create \
    "$root/target/x86_64-apple-darwin/release/libwavelite.a" \
    "$root/target/aarch64-apple-darwin/release/libwavelite.a" \
    -output "$universal/libwavelite.a"
lipo -info "$universal/libwavelite.a"

export WAVELITE_CORE_LIBRARY_DIR="$universal"
swift build \
    --package-path "$root/macos" \
    --configuration release \
    --arch arm64 \
    --arch x86_64
bin_path=$(swift build \
    --package-path "$root/macos" \
    --configuration release \
    --arch arm64 \
    --arch x86_64 \
    --show-bin-path)
executable="$bin_path/WaveLite"
test -s "$executable"
lipo -info "$executable"

app="$root/target/macos/WaveLite.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$executable" "$app/Contents/MacOS/WaveLite"
cp "$root/packaging/macos/Info.plist" "$app/Contents/Info.plist"
cp "$root/app-icons/macos/compiled/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
cp "$root/app-icons/macos/compiled/Assets.car" "$app/Contents/Resources/Assets.car"
plutil -lint "$app/Contents/Info.plist"
lipo -info "$app/Contents/MacOS/WaveLite"
echo "Built $app"
