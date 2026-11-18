#[cfg(any(target_os = "macos", test))]
uniffi::setup_scaffolding!();

pub mod cli;
pub mod controller;
#[cfg(any(feature = "demo", target_os = "macos", test))]
pub mod demo;
pub mod device;
#[cfg(feature = "gui")]
pub mod gtk4;
#[cfg(any(target_os = "macos", test))]
pub mod macos;
pub mod protocol;
pub mod wave3_mk2;
#[cfg(all(target_os = "windows", feature = "windows-gui"))]
pub mod win;
