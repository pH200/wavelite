use std::{path::PathBuf, process::ExitCode, time::Duration};

use wavelite::{demo, device::DeviceModel, gtk4};

fn main() -> ExitCode {
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

    let model = model_from_env().unwrap_or(DeviceModel::Wave3Mk2);
    let controller = demo::controller(model, Duration::ZERO);
    if let Some(path) = screenshot.clone() {
        gtk4::run_demo_with_screenshot(controller, wave_fx_page(), path);
    } else {
        gtk4::run_demo(controller, wave_fx_page());
    }

    if let Some(path) = screenshot {
        if !path.is_file() {
            eprintln!("No screenshot was created at {}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn model_from_env() -> Option<DeviceModel> {
    match std::env::var("WAVELITE_FAKE_MODEL").ok()?.as_str() {
        "wave3-mk1" => Some(DeviceModel::Wave3),
        "wave3-mk2" => Some(DeviceModel::Wave3Mk2),
        _ => None,
    }
}

fn wave_fx_page() -> Option<u32> {
    match std::env::var("WAVELITE_FAKE_WAVE_FX").ok()?.as_str() {
        "dynamics" => Some(0),
        "eq" => Some(1),
        _ => None,
    }
}
