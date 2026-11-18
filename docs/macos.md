# macOS build

Use the separate build scripts to produce a universal CLI at `target/release/wavelite` and a native Swift/AppKit app at `target/macos/WaveLite.app`, both with `x86_64` and `arm64` slices:

```sh
./packaging/macos/build-cli.sh
./packaging/macos/build-app.sh
```

Rustup, Swift, and Apple's Command Line Tools are required. The scripts install both Rust targets and build libusb from the vendored crate, so no Homebrew packages are needed. No `codesign` command, certificate, or signing identity is used. On arm64, the linker may emit the anonymous ad-hoc signature required by macOS; no identity is attached. Notarization is not performed. `install.sh` remains Linux-only, and a direct `cargo build` targets only the host architecture.

## Native GUI

The macOS app provides the same common controls, dial functions, serial privacy, reconnect/error handling, and MK.2 DSP/EQ editor through native AppKit controls. Build and launch it with:

```sh
./packaging/macos/build-app.sh
open target/macos/WaveLite.app
```

Run the fake-device UI matrix with `./packaging/macos/test-app.sh`. It verifies seven MK.1 settings and eighteen MK.2 settings with 150 ms injected I/O latency and creates four screenshots under `target/screenshots/macos-native/`.

## Checks

Run the Rust checks:

```sh
cargo test --locked
cargo test --locked --no-default-features
cargo test --locked --all-features
```

The normal debug Rust build produces the static library needed by SwiftPM. Run the additional checks with:

```sh
cargo build --locked --no-default-features
swift test --package-path macos
./packaging/macos/build-cli.sh
./packaging/macos/build-app.sh
./packaging/macos/test-app.sh
```

The macOS app uses checked-in UniFFI 0.32.2 Swift bindings with typed settings, events, and configuration records. After changing the exported bridge in `src/macos/mod.rs` or `src/macos/types.rs`, regenerate them with `./packaging/macos/generate-bindings.sh`; see the [macOS GUI notes](macos-gui.md) for regeneration instructions and details about the UniFFI bridge between Swift/AppKit and Rust.
