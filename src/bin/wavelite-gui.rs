#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(all(not(target_os = "windows"), feature = "gui"))]
fn main() {
    if let Some(path) = std::env::var_os("WAVELITE_SCREENSHOT") {
        wavelite::gtk4::run_with_screenshot(wavelite::controller::usb(), None, path.into());
    } else {
        wavelite::gtk4::run(wavelite::controller::usb(), None);
    }
}

#[cfg(all(target_os = "windows", feature = "windows-gui"))]
fn main() {
    let options = wavelite::win::Options {
        screenshot: std::env::var_os("WAVELITE_SCREENSHOT").map(std::path::PathBuf::from),
        ..Default::default()
    };
    if let Err(error) = wavelite::win::run(wavelite::controller::usb(), options) {
        eprintln!("WaveLite native GUI failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(all(not(target_os = "windows"), not(feature = "gui")))]
fn main() {}

#[cfg(all(target_os = "windows", not(feature = "windows-gui")))]
fn main() {}
