# macOS GUI

The macOS GUI uses UniFFI 0.32.2 to connect Swift/AppKit with Rust.

- Rust bridge: `src/macos/mod.rs`
- Rust types: `src/macos/types.rs`
- Generated Swift bindings: `macos/Sources/WaveLiteKit/WaveLiteKit.swift` and `macos/Sources/WaveLiteCoreFFI/`

After changing the bridge or types, regenerate the checked-in bindings on macOS. Install the matching generator once, then build the Rust library and run the generator:

```sh
cargo install uniffi --version 0.32.2 --features cli --locked
cargo build --locked --no-default-features
./packaging/macos/generate-bindings.sh
```

The generator updates the Swift bindings and FFI header/module map. Do not edit generated files manually.
