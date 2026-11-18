# Wavelite app icons

Design 02 ("pulse"). The GNOME files use WaveLite's application ID, `dev.wavelite.WaveLite`.

## GNOME

| File | Install to | Used for |
|---|---|---|
| `gnome/hicolor/scalable/apps/dev.wavelite.WaveLite.svg` | `$prefix/share/icons/hicolor/scalable/apps/` | App icon (128 grid, 4px lip on the template baseline) |
| `gnome/hicolor/symbolic/apps/dev.wavelite.WaveLite-symbolic.svg` | `$prefix/share/icons/hicolor/symbolic/apps/` | Symbolic app icon (16px, recoloured by GTK) |
| `gnome/hicolor/symbolic/status/dev.wavelite.WaveLite-tray-symbolic.svg` | `$prefix/share/icons/hicolor/symbolic/status/` | Top-bar icon via AppIndicator / StatusNotifierItem |

Desktop file: `Icon=dev.wavelite.WaveLite`. Tray: set the indicator's icon name to `dev.wavelite.WaveLite-tray-symbolic`
(GNOME Shell only shows tray icons with the AppIndicator extension, which Ubuntu ships by default).

## macOS

**Xcode project**
- `macos/AppIcon.icon` — Icon Composer document (square unmasked layers; the system adds mask, Liquid Glass, shadow,
  and dark/clear/tinted variants). Drag into the project; the target's App Icon name must be `AppIcon`.
  Xcode generates icons for pre-Tahoe releases automatically.
- `macos/Assets.xcassets/MenuBarIcon.imageset` — 18×18 pt vector template image. Merge into your asset catalog, then
  `MenuBarExtra("Wavelite", image: "MenuBarIcon")` or `statusItem.button?.image = NSImage(named: "MenuBarIcon")`.

**Other build systems (Tauri, Electron, CMake…)**
- `macos/compiled/Assets.car` + `macos/compiled/AppIcon.icns` → `Contents/Resources/`, with Info.plist keys
  `CFBundleIconName = AppIcon` (Liquid Glass icon on macOS 26+) and `CFBundleIconFile = AppIcon` (older macOS).
  Rebuild after editing the `.icon`:
  ```sh
  xcrun actool macos/AppIcon.icon macos/Assets.xcassets --compile macos/compiled --platform macosx \
    --minimum-deployment-target 11.0 --app-icon AppIcon --standalone-icon-behavior all \
    --output-partial-info-plist /tmp/partial.plist
  ```
- `macos/MenuBarIconTemplate.png` / `@2x.png` — the `Template` suffix makes AppKit and Electron tint it for the menu bar.

## Windows

`windows/WaveLite.ico` contains the app icon at 16, 20, 24, 32, 40, 48, 64, 128, and 256 pixels. The native GUI embeds it in its executables, and Windows uses it for the windows, taskbar, and shortcuts.
