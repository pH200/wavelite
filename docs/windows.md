# Windows build

Build on 64-bit Windows from an MSVC Developer PowerShell with Rust 1.95 or newer and the MSVC C++ Build Tools installed:

```powershell
cargo build --release --locked --no-default-features --features windows-gui --bins
```

The CLI and native GUI are written to `target\release\`. The Windows build uses the MSVC resource compiler to embed the application icon and Per-Monitor V2 DPI-awareness manifest and does not require GTK. Native controls use the system UI font metrics for the window's DPI rather than a fixed font family or point size.

Release packages are available for x86_64 and ARM64. The MSI installs only the GUI; the CLI is distributed in a separate ZIP archive.

## Checks

```powershell
cargo test --locked
cargo test --locked --no-default-features
cargo test --locked --all-features
```
