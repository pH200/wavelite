#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$root/Cargo.toml")
architecture=$(uname -m)

if [[ -z "$version" ]]; then
    echo "error: could not read the package version from Cargo.toml" >&2
    exit 1
fi
for command in cargo curl pkg-config sha256sum find sed glib-compile-schemas; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "error: $command is required to build the AppImage" >&2
        exit 1
    fi
done
if ! pkg-config --exists 'gtk4 >= 4.10' libusb-1.0 gdk-pixbuf-2.0; then
    echo "error: GTK4 >= 4.10, libusb, and gdk-pixbuf development files are required" >&2
    exit 1
fi

case "$architecture" in
    x86_64)
        linuxdeploy_sha256=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
        runtime_sha256=617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46
        ;;
    aarch64)
        linuxdeploy_sha256=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff
        runtime_sha256=9a72a744d221576c9347ec074243acc5c3c7568896b12714e0e82e9f0bf47900
        ;;
    *)
        echo "error: AppImage builds currently support x86_64 and aarch64, not $architecture" >&2
        exit 1
        ;;
esac

linuxdeploy_version=1-alpha-20251107-1
tools_dir="$root/target/appimage-tools"
linuxdeploy="$tools_dir/linuxdeploy-$linuxdeploy_version-$architecture.AppImage"
runtime="$tools_dir/runtime-fedfad51acc2860b2916a1eef32ba4a500ff31ef-$architecture"
appdir="$root/target/appimage/$architecture/WaveLite.AppDir"
output="$root/target/release/wavelite-$version-linux-$architecture.AppImage"

mkdir -p "$tools_dir" "$(dirname -- "$output")"
if [[ ! -x "$linuxdeploy" ]] || ! echo "$linuxdeploy_sha256  $linuxdeploy" | sha256sum -c --status; then
    temporary="$linuxdeploy.download"
    rm -f "$temporary"
    curl --fail --location --retry 3 \
        --output "$temporary" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/$linuxdeploy_version/linuxdeploy-$architecture.AppImage"
    echo "$linuxdeploy_sha256  $temporary" | sha256sum -c -
    chmod 0755 "$temporary"
    mv "$temporary" "$linuxdeploy"
fi
if [[ ! -f "$runtime" ]] || ! echo "$runtime_sha256  $runtime" | sha256sum -c --status; then
    temporary="$runtime.download"
    rm -f "$temporary"
    curl --fail --location --retry 3 \
        --output "$temporary" \
        "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$architecture"
    echo "$runtime_sha256  $temporary" | sha256sum -c -
    mv "$temporary" "$runtime"
fi

cargo build --manifest-path "$root/Cargo.toml" \
    --release --locked --bin wavelite --bin wavelite-gui

rm -rf "$appdir"
mkdir -p "$appdir/usr/share/glib-2.0" "$appdir/usr/share/doc/wavelite"
cp "$root/LICENSE" "$appdir/usr/share/doc/wavelite/copyright"
schemas_dir=$(pkg-config --variable=schemasdir gio-2.0)
schemas_dir=${schemas_dir:-/usr/share/glib-2.0/schemas}
if ! compgen -G "$schemas_dir/*.xml" >/dev/null && [[ -d /usr/share/glib-2.0/schemas ]]; then
    schemas_dir=/usr/share/glib-2.0/schemas
fi
if [[ -d "$schemas_dir" ]]; then
    cp -aL "$schemas_dir" "$appdir/usr/share/glib-2.0/schemas"
    if compgen -G "$appdir/usr/share/glib-2.0/schemas/*.xml" >/dev/null; then
        glib-compile-schemas "$appdir/usr/share/glib-2.0/schemas"
    fi
fi

module_libraries=()
mkdir -p "$appdir/usr/lib"
gtk_module_dir="$(pkg-config --variable=libdir gtk4)/gtk-4.0"
if [[ -d "$gtk_module_dir" ]]; then
    cp -aL "$gtk_module_dir" "$appdir/usr/lib/gtk-4.0"
    while IFS= read -r -d '' module; do
        module_libraries+=("--library=$module")
    done < <(find -L "$gtk_module_dir" -type f -name '*.so' -print0)
fi

gdk_pixbuf_binarydir=$(pkg-config --variable=gdk_pixbuf_binarydir gdk-pixbuf-2.0)
gdk_pixbuf_moduledir=$(pkg-config --variable=gdk_pixbuf_moduledir gdk-pixbuf-2.0)
if [[ -z "$gdk_pixbuf_binarydir" || -z "$gdk_pixbuf_moduledir" ]]; then
    echo "error: gdk-pixbuf did not report its module directory" >&2
    exit 1
fi
gdk_pixbuf_version=$(basename -- "$gdk_pixbuf_binarydir")
pixbuf_destination="$appdir/usr/lib/gdk-pixbuf-2.0/$gdk_pixbuf_version"
mkdir -p "$(dirname -- "$pixbuf_destination")"
cp -aL "$gdk_pixbuf_binarydir" "$pixbuf_destination"
while IFS= read -r -d '' module; do
    module_libraries+=("--library=$module")
done < <(find -L "$gdk_pixbuf_moduledir" -type f -name '*.so' -print0)

libusb_library="$(pkg-config --variable=libdir libusb-1.0)/libusb-1.0.so.0"
if [[ ! -f "$libusb_library" ]]; then
    echo "error: libusb library not found: $libusb_library" >&2
    exit 1
fi
module_libraries+=("--library=$libusb_library")

gdk_pixbuf_query=$(pkg-config --variable=gdk_pixbuf_query_loaders gdk-pixbuf-2.0)
if [[ -z "$gdk_pixbuf_query" ]] || [[ ! -x "$gdk_pixbuf_query" ]]; then
    gdk_pixbuf_query=$(command -v gdk-pixbuf-query-loaders || true)
fi
if [[ -z "$gdk_pixbuf_query" ]]; then
    echo "error: gdk-pixbuf-query-loaders is required to build the AppImage" >&2
    exit 1
fi
"$gdk_pixbuf_query" > "$pixbuf_destination/loaders.cache"
sed -i "s|$gdk_pixbuf_moduledir/||g" "$pixbuf_destination/loaders.cache"
chmod -R u+w "$appdir"

rm -f "$output" "$output.zsync"
export APPIMAGE_EXTRACT_AND_RUN=1
export ARCH="$architecture"
export LINUXDEPLOY_OUTPUT_VERSION="$version"
export LDAI_OUTPUT="$output"
export LDAI_RUNTIME_FILE="$runtime"
export LDAI_NO_APPSTREAM=1
export LDAI_UPDATE_INFORMATION="gh-releases-zsync|pH200|wavelite|latest|wavelite-*-linux-$architecture.AppImage.zsync"
(
    cd "$(dirname -- "$output")"
    "$linuxdeploy" \
        --appdir "$appdir" \
        --executable "$root/target/release/wavelite" \
        --executable "$root/target/release/wavelite-gui" \
        --desktop-file "$root/packaging/dev.wavelite.WaveLite.desktop" \
        --icon-file "$root/app-icons/gnome/hicolor/scalable/apps/dev.wavelite.WaveLite.svg" \
        --custom-apprun "$root/packaging/linux/AppRun" \
        "${module_libraries[@]}" \
        --output appimage
)

if [[ ! -s "$output" ]]; then
    echo "error: linuxdeploy did not create $output" >&2
    exit 1
fi
if [[ ! -s "$output.zsync" ]]; then
    echo "error: linuxdeploy did not create $output.zsync" >&2
    exit 1
fi
if [[ ! -e "$appdir/usr/lib/libusb-1.0.so.0" ]]; then
    echo "error: linuxdeploy did not bundle libusb" >&2
    exit 1
fi
ls -lh "$output" "$output.zsync"
