#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    use std::{path::PathBuf, process::ExitCode, time::Duration};
    use wavelite::{demo, device::DeviceModel, win};

    let screenshot = std::env::var_os("WAVELITE_SCREENSHOT").map(PathBuf::from);
    if let Some(path) = &screenshot {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                eprintln!("Could not replace {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let model = match std::env::var("WAVELITE_FAKE_MODEL").ok().as_deref() {
        Some("wave3-mk1") => DeviceModel::Wave3,
        Some("wave3-mk2") => DeviceModel::Wave3Mk2,
        _ => DeviceModel::Wave3Mk2,
    };
    let responsiveness_test = std::env::var_os("WAVELITE_NATIVE_RESPONSIVENESS_TEST").is_some();
    let delay = std::env::var("WAVELITE_FAKE_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(if responsiveness_test { 150 } else { 0 });
    let wave_fx_page = match std::env::var("WAVELITE_FAKE_WAVE_FX").ok().as_deref() {
        Some("dynamics") => Some(0),
        Some("eq") => Some(1),
        _ => None,
    };
    let options = win::Options {
        screenshot: screenshot.clone(),
        responsiveness_test,
        wave_fx_page,
    };
    if let Err(error) = win::run(
        demo::controller(model, Duration::from_millis(delay)),
        options,
    ) {
        eprintln!("WaveLite native demo failed: {error}");
        return ExitCode::FAILURE;
    }
    if let Some(path) = screenshot {
        if !path.is_file() {
            eprintln!("No screenshot was created at {}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

#[cfg(not(target_os = "windows"))]
fn main() {}
