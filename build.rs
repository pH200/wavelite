use std::{env, ffi::OsString, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=packaging/windows/wavelite.rc");
    println!("cargo:rerun-if-changed=packaging/windows/wavelite.manifest");
    println!("cargo:rerun-if-changed=app-icons/windows/WaveLite.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var_os("CARGO_FEATURE_WINDOWS_GUI").is_none()
    {
        return;
    }

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("wavelite.res");
    let compiler = env::var_os("RC").unwrap_or_else(|| OsString::from("rc.exe"));
    let status = Command::new(compiler)
        .current_dir("packaging/windows")
        .arg("/nologo")
        .arg(format!("/fo{}", output.display()))
        .arg("wavelite.rc")
        .status()
        .expect("failed to run the Windows resource compiler (set RC to its path)");
    assert!(status.success(), "Windows resource compilation failed");

    for binary in ["wavelite-gui", "wavelite-native-gui-demo"] {
        println!("cargo:rustc-link-arg-bin={binary}={}", output.display());
    }
}
