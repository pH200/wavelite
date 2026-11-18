#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

local_install=false
with_udev=true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --local)
            local_install=true
            ;;
        --no-udev)
            with_udev=false
            ;;
        -h|--help)
            echo "usage: $0 [--local] [--no-udev]"
            exit 0
            ;;
        *)
            echo "usage: $0 [--local] [--no-udev]" >&2
            exit 2
            ;;
    esac
    shift
done

if ! $local_install || $with_udev; then
    if [[ $EUID -ne 0 ]] && ! command -v sudo >/dev/null 2>&1; then
        echo "error: sudo is required for system installation and udev setup; use --local --no-udev to install without it" >&2
        exit 1
    fi
fi

privilege=()
if [[ $EUID -ne 0 ]]; then
    privilege=(sudo)
fi

cargo=$(command -v cargo 2>/dev/null || true)
rust_command=$cargo
if [[ -z "$rust_command" ]]; then
    rust_command=$(command -v rustup 2>/dev/null || true)
fi

brew_rust=false
brew_prefix=
if command -v brew >/dev/null 2>&1; then
    brew_prefix=$(brew --prefix)
    if [[ "$rust_command" == "$brew_prefix/bin/cargo" \
        || "$rust_command" == "$brew_prefix/opt/rust/bin/cargo" \
        || "$rust_command" == "$brew_prefix/opt/rustup/bin/cargo" \
        || "$rust_command" == "$brew_prefix/bin/rustup" \
        || "$rust_command" == "$brew_prefix/opt/rustup/bin/rustup" ]]; then
        brew_rust=true
    fi
fi

if [[ -z "$cargo" ]] && command -v rustup >/dev/null 2>&1; then
    cargo=$(rustup which cargo 2>/dev/null || true)
fi
if [[ -z "$cargo" ]]; then
    echo "error: cargo not found; install a Rust toolchain with 'rustup toolchain install stable'" >&2
    exit 1
fi

pkg_config=$(command -v pkg-config 2>/dev/null || true)
if $brew_rust; then
    pkg_config="$brew_prefix/bin/pkg-config"
    export PKG_CONFIG="$pkg_config"
    export PKG_CONFIG_LIBDIR="$brew_prefix/lib/pkgconfig:$brew_prefix/share/pkgconfig"
    unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR
    RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-Wl,-rpath,$brew_prefix/lib"
    export RUSTFLAGS
fi

if [[ -z "$pkg_config" || ! -x "$pkg_config" ]] || ! "$pkg_config" --exists 'gtk4 >= 4.10' libusb-1.0; then
    echo "error: GTK4 >= 4.10 and libusb development files are required" >&2
    if $brew_rust; then
        echo "Homebrew: brew install pkg-config libusb gtk4" >&2
    else
        distro=
        if [[ -r /etc/os-release ]]; then
            . /etc/os-release
            distro=" ${ID:-} ${ID_LIKE:-} ${VARIANT_ID:-} "
        fi
        case "$distro" in
            *bazzite*)
                echo "Bazzite: brew install pkg-config libusb gtk4" >&2
                ;;
            *silverblue*|*kinoite*|*sericea*|*atomic*|*coreos*)
                echo "Fedora Atomic: sudo rpm-ostree install gcc pkgconf-pkg-config libusb1-devel gtk4-devel (then reboot)" >&2
                ;;
            *debian*|*ubuntu*)
                echo "Debian/Ubuntu: sudo apt install build-essential pkg-config libusb-1.0-0-dev libgtk-4-dev" >&2
                ;;
            *fedora*)
                echo "Fedora: sudo dnf install gcc pkgconf-pkg-config libusb1-devel gtk4-devel" >&2
                ;;
            *arch*)
                echo "Arch: sudo pacman -S --needed base-devel pkgconf libusb gtk4" >&2
                ;;
            *)
                echo "Install pkg-config plus the GTK4 and libusb development packages for your distribution." >&2
                ;;
        esac
    fi
    exit 1
fi

PATH="$(dirname -- "$cargo"):$PATH"
export PATH
"$cargo" build --manifest-path "$root/Cargo.toml" --release --locked --bins

if $local_install; then
    bin_dir="$root/.local/bin"
    bin_install_dir=$bin_dir
    bin_privilege=()
else
    bin_dir=/usr/local/bin
    bin_install_dir=$(readlink -m -- "$bin_dir")
    bin_privilege=("${privilege[@]}")
fi

"${bin_privilege[@]}" install -d "$bin_install_dir"
for binary in wavelite wavelite-gui; do
    "${bin_privilege[@]}" install -m 0755 "$root/target/release/$binary" "$bin_install_dir/$binary"
done
echo "Installed commands in $bin_dir"

if $local_install; then
    data_dir="$root/.local/share"
    data_privilege=()
else
    data_dir=/usr/local/share
    data_privilege=("${privilege[@]}")
fi
"${data_privilege[@]}" install -d \
    "$data_dir/applications" \
    "$data_dir/icons/hicolor/scalable/apps" \
    "$data_dir/icons/hicolor/symbolic/apps" \
    "$data_dir/icons/hicolor/symbolic/status"
"${data_privilege[@]}" install -m 0644 \
    "$root/packaging/dev.wavelite.WaveLite.desktop" \
    "$data_dir/applications/dev.wavelite.WaveLite.desktop"
"${data_privilege[@]}" install -m 0644 \
    "$root/app-icons/gnome/hicolor/scalable/apps/dev.wavelite.WaveLite.svg" \
    "$data_dir/icons/hicolor/scalable/apps/dev.wavelite.WaveLite.svg"
"${data_privilege[@]}" install -m 0644 \
    "$root/app-icons/gnome/hicolor/symbolic/apps/dev.wavelite.WaveLite-symbolic.svg" \
    "$data_dir/icons/hicolor/symbolic/apps/dev.wavelite.WaveLite-symbolic.svg"
"${data_privilege[@]}" install -m 0644 \
    "$root/app-icons/gnome/hicolor/symbolic/status/dev.wavelite.WaveLite-tray-symbolic.svg" \
    "$data_dir/icons/hicolor/symbolic/status/dev.wavelite.WaveLite-tray-symbolic.svg"
echo "Installed desktop launcher and icons in $data_dir"

if $with_udev; then
    rule_dir=/etc/udev/rules.d
    "${privilege[@]}" install -d "$rule_dir"
    "${privilege[@]}" install -m 0644 "$root/packaging/99-wavelite.wavemic.rules" "$rule_dir/99-wavelite.wavemic.rules"
    echo "Installed udev rule in $rule_dir"

    udevadm=$(command -v udevadm 2>/dev/null || true)
    if [[ -z "$udevadm" ]]; then
        echo "warning: udevadm not found; reboot or reload udev rules to activate the installed rule" >&2
    elif ! "${privilege[@]}" "$udevadm" control --reload-rules; then
        echo "warning: could not reload udev rules; reboot or reload them manually to activate the installed rule" >&2
    else
        for product in 006e 0070 00ca; do
            if ! "${privilege[@]}" "$udevadm" trigger --subsystem-match=usb --attr-match=idVendor=0fd9 --attr-match="idProduct=$product"; then
                echo "warning: could not trigger udev; reconnect the microphone after installation" >&2
                break
            fi
        done
    fi
fi

"$bin_dir/wavelite" --help
