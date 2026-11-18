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
if ! command -v lipo >/dev/null 2>&1; then
    echo "error: lipo not found; install Apple's Command Line Tools" >&2
    exit 1
fi
if command -v rustup >/dev/null 2>&1; then
    rustup target add "${targets[@]}"
fi

PATH="$(dirname -- "$cargo"):$PATH"
export PATH
for target in "${targets[@]}"; do
    "$cargo" build \
        --manifest-path "$root/Cargo.toml" \
        --target "$target" \
        --release \
        --locked \
        --no-default-features \
        --bin wavelite
done

mkdir -p "$root/target/release"
lipo -create \
    "$root/target/x86_64-apple-darwin/release/wavelite" \
    "$root/target/aarch64-apple-darwin/release/wavelite" \
    -output "$root/target/release/wavelite"
lipo -info "$root/target/release/wavelite"
