# Linux build and installation

The `wavelite` command and GTK GUI install into `/usr/local/bin` by default. The installer also installs a udev rule that lets your normal user account access the microphone's USB controls. On the first Linux installation, sudo is required to copy the app into system directories and install this rule. WaveLite itself does not run as root. No kernel module, DKMS build, system Python package, or OSTree package layering is required.

Install Rust with [rustup](https://rustup.rs/) if it is not already available. The installer supports rustup installations that do not put a `cargo` shim on `PATH`, including Linuxbrew rustup. It does not install build dependencies; when they are missing, it prints the appropriate package command for the detected distribution.

## Build and install

Build, test, and install:

```sh
cd /path/to/wavelite
cargo test --locked
./install.sh
```

For a project-local installation in the checkout, add `--local`. The installer still installs the udev rule by default, so this mode also uses sudo:

```sh
./install.sh --local
```

The installer adds the WaveLite desktop launcher and GNOME icons under `/usr/local/share`, or under the checkout's `.local/share` for a project-local installation.

To skip udev setup, use `--no-udev`. You can combine it with `--local` to run the installer entirely without sudo, but you must grant USB access separately before WaveLite can control real hardware:

```sh
./install.sh --local --no-udev
```

The default rule covers exactly `0fd9:006e`, `0fd9:0070`, and `0fd9:00ca`, grants access to `plugdev` and the active desktop user through `uaccess`, and leaves other USB devices unchanged. Reconnect the microphone if access is still denied. On a headless Debian or Ubuntu system without an active desktop seat, also add the intended user to `plugdev` with `sudo usermod -a plugdev USERNAME` and log in again.

### Manual udev setup and installation without sudo

You can install the USB permission rule yourself, then build WaveLite directly without running the installer. From the project checkout:

```sh
sudo install -d /etc/udev/rules.d
sudo install -m 0644 packaging/99-wavelite.wavemic.rules /etc/udev/rules.d/99-wavelite.wavemic.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=usb --attr-match=idVendor=0fd9
cargo build --release --locked --bins
```

Reconnect the microphone after installing the rule. The build writes `wavelite` and `wavelite-gui` to `target/release/`; move the binaries to a user-writable location on your `PATH` yourself. Neither the build nor this manual placement requires sudo.

### Native build dependencies

Debian and Ubuntu:

```sh
sudo apt update
sudo apt install build-essential pkg-config libusb-1.0-0-dev libgtk-4-dev
```

Fedora:

```sh
sudo dnf install gcc pkgconf-pkg-config libusb1-devel gtk4-devel
```

Arch Linux:

```sh
sudo pacman -S --needed base-devel pkgconf libusb gtk4
```

GTK4 4.10 or newer is required. Debian 12 provides GTK4 4.8, so use Debian 13 or newer, Ubuntu 24.04 or newer, or Linuxbrew on older releases.

### Linuxbrew

Install native prerequisites with:

```sh
brew install pkg-config libusb gtk4
```

Linuxbrew builds use Homebrew's `pkg-config` and development packages. The installer records Homebrew's library directory in the commands' runtime search path so they also work outside a Brew-configured shell.

### Fedora Atomic and Bazzite

Do not unlock OSTree or layer the application. `/usr/local/bin` and `/etc/udev/rules.d` persist across upgrades.

On Fedora Atomic, install the native development prerequisites and reboot before building:

```sh
sudo rpm-ostree install gcc pkgconf-pkg-config libusb1-devel gtk4-devel
```

On Bazzite, use Linuxbrew instead:

```sh
brew install rust pkg-config libusb gtk4
```

A Distrobox build also works when its binary uses libraries available on the host. Running WaveLite inside Distrobox does not remove the need for host-side USB permission.

## Linux AppImage

Build an x86_64 or aarch64 AppImage on the corresponding Linux architecture with the native build dependencies and `curl` installed:

```sh
./packaging/linux/build-appimage.sh
```

The script downloads pinned, SHA-256-verified AppImage build tools into `target/appimage-tools/` and writes `target/release/wavelite-VERSION-linux-ARCH.AppImage` together with `target/release/wavelite-VERSION-linux-ARCH.AppImage.zsync`. The AppImage contains the GTK4 GUI, CLI, libusb, shared libraries, GLib schemas, GTK modules, and image loaders. Official x86_64 releases are built with Ubuntu 24.04 system libraries. For manual builds, use the oldest distribution release you intend to support because some host system libraries are not bundled.

The AppImage embeds `gh-releases-zsync` update information for the latest stable release in `pH200/wavelite`, with a separate filename pattern for each architecture. Official releases currently include x86_64 binaries only; aarch64 builds and their update metadata remain supported for manual builds. Upload each released AppImage and its generated `.AppImage.zsync` file together as assets of the same GitHub release, keeping their original filenames. [Gear Lever](https://github.com/mijorus/gearlever) and other AppImage update tools use these files to check for and download updates. The `latest` update channel ignores prereleases.

### Use the AppImage

Download or copy the AppImage for your architecture, make it executable, and run it. The filename includes the WaveLite version and architecture; replace the example filename with the file you have:

```sh
chmod +x wavelite-0.1.0-linux-x86_64.AppImage
./wavelite-0.1.0-linux-x86_64.AppImage
```

Launching the AppImage without arguments opens the GUI. Pass `--cli` before any WaveLite command to use the bundled command-line interface instead:

```sh
./wavelite-0.1.0-linux-x86_64.AppImage --cli info
./wavelite-0.1.0-linux-x86_64.AppImage --cli --help
```

The AppImage does not install a desktop launcher or USB permissions. If the GUI reports that the USB access rule is missing or outdated, use **Install USB Access Rule…**. Rule installation uses the host's `pkexec`/polkit and `udevadm`; these system tools are not bundled. Reconnect the microphone after installing the rule if it does not appear.

## Headless CLI-only build

For a headless CLI-only build without GTK:

```sh
cargo build --release --locked --no-default-features --bin wavelite
```

## Checks

```sh
cargo test --locked
cargo test --locked --no-default-features
cargo test --locked --all-features
```
