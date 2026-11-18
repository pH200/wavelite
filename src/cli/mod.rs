use std::{fs, path::PathBuf};

use crate::{
    device::{DeviceConfig, DeviceConfigView, DeviceError, Setting, UsbTransport, WaveDevice},
    protocol::{Config, DeviceInfoView, DialMode, ProtocolError, decode_hex, format_hex},
    wave3_mk2::{ClipguardMode, DynamicsUpdate, EqBandType, EqBandUpdate, Wave3Mk2Config},
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "wavelite", disable_version_flag = true)]
struct Cli {
    #[arg(long, global = true, help = "Select a specific Wave serial number")]
    serial: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Info,
    Dial {
        #[arg(value_enum)]
        mode: DialArgument,
    },
    Clipguard {
        #[arg(value_enum)]
        state: Toggle,
    },
    ClipguardMode {
        #[arg(value_enum)]
        mode: ClipguardModeArgument,
    },
    Lowcut {
        #[arg(value_enum)]
        state: Toggle,
    },
    Gain {
        db: f64,
    },
    Mute {
        #[arg(value_enum)]
        state: Toggle,
    },
    OutputVolume {
        db: f64,
    },
    MonitorMix {
        percent: f64,
    },
    Expander {
        #[arg(value_enum)]
        state: Toggle,
    },
    VoiceTune {
        #[arg(value_enum)]
        state: Toggle,
    },
    VoiceTuneStrength {
        percent: f64,
    },
    Compressor {
        #[arg(value_enum)]
        state: Toggle,
    },
    Eq {
        #[command(subcommand)]
        command: EqCommand,
    },
    Dynamics {
        #[command(subcommand)]
        command: DynamicsCommand,
    },
    Snapshot {
        path: PathBuf,
    },
    Restore {
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum EqCommand {
    Enabled {
        #[arg(value_enum)]
        state: Toggle,
    },
    Band {
        index: usize,
        #[arg(long, value_enum)]
        enabled: Option<Toggle>,
        #[arg(long, value_enum)]
        kind: Option<EqBandTypeArgument>,
        #[arg(long, allow_negative_numbers = true)]
        gain_db: Option<f64>,
        #[arg(long)]
        frequency_hz: Option<f64>,
        #[arg(long)]
        q: Option<f64>,
    },
}

#[derive(Debug, Subcommand)]
enum DynamicsCommand {
    Compressor {
        #[arg(long, allow_negative_numbers = true)]
        threshold_db: Option<f64>,
        #[arg(long)]
        ratio: Option<f64>,
        #[arg(long)]
        attack_ms: Option<f64>,
        #[arg(long)]
        release_ms: Option<f64>,
        #[arg(long)]
        makeup_gain_db: Option<f64>,
    },
    Expander {
        #[arg(long, allow_negative_numbers = true)]
        threshold_db: Option<f64>,
        #[arg(long)]
        ratio: Option<f64>,
        #[arg(long)]
        attack_ms: Option<f64>,
        #[arg(long)]
        release_ms: Option<f64>,
        #[arg(long)]
        max_reduction_db: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DialArgument {
    InputGain,
    OutputVolume,
    MonitorMix,
}

impl From<DialArgument> for DialMode {
    fn from(value: DialArgument) -> Self {
        match value {
            DialArgument::InputGain => Self::InputGain,
            DialArgument::OutputVolume => Self::OutputVolume,
            DialArgument::MonitorMix => Self::MonitorMix,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ClipguardModeArgument {
    Enabled,
    Bypassed,
    HardClip,
}

impl From<ClipguardModeArgument> for ClipguardMode {
    fn from(value: ClipguardModeArgument) -> Self {
        match value {
            ClipguardModeArgument::Enabled => Self::Enabled,
            ClipguardModeArgument::Bypassed => Self::Bypassed,
            ClipguardModeArgument::HardClip => Self::HardClip,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EqBandTypeArgument {
    HighPass,
    LowShelf,
    Bell,
    Notch,
    HighShelf,
    LowPass,
}

impl From<EqBandTypeArgument> for EqBandType {
    fn from(value: EqBandTypeArgument) -> Self {
        match value {
            EqBandTypeArgument::HighPass => Self::HighPass,
            EqBandTypeArgument::LowShelf => Self::LowShelf,
            EqBandTypeArgument::Bell => Self::Bell,
            EqBandTypeArgument::Notch => Self::Notch,
            EqBandTypeArgument::HighShelf => Self::HighShelf,
            EqBandTypeArgument::LowPass => Self::LowPass,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Toggle {
    On,
    Off,
}

impl From<Toggle> for bool {
    fn from(value: Toggle) -> Self {
        matches!(value, Toggle::On)
    }
}

#[derive(Debug, Serialize)]
struct State {
    device: DeviceInfoView,
    model: String,
    config: DeviceConfigView,
    #[serde(skip_serializing_if = "Option::is_none")]
    meter_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SavedState {
    model: Option<String>,
    config: SavedConfig,
}

#[derive(Debug, Deserialize)]
struct SavedConfig {
    raw_hex: Option<String>,
    settings_hex: Option<String>,
    headphones_hex: Option<String>,
    monitor_hex: Option<String>,
}

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Device(#[from] DeviceError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn run() -> u8 {
    let cli = Cli::parse();
    match execute(cli) {
        Ok(()) => 0,
        Err(error) => {
            let message = error.to_string();
            eprintln!("wavelite: {message}");
            if message.contains("Access denied") || message.contains("insufficient permissions") {
                eprintln!(
                    "Install packaging/99-wavelite.wavemic.rules with ./install.sh, then reconnect the microphone."
                );
            }
            1
        }
    }
}

fn execute(cli: Cli) -> Result<(), CliError> {
    let mut device = WaveDevice::new(UsbTransport::open(cli.serial.as_deref())?);
    match cli.command {
        Command::Info => print_json(&state(&mut device)?)?,
        Command::Snapshot { path } => {
            let state = state(&mut device)?;
            let text = json_text(&state)?;
            fs::write(path, &text)?;
            print!("{text}");
        }
        Command::Restore { path } => {
            let saved: SavedState = serde_json::from_str(&fs::read_to_string(path)?)?;
            if let Some(model) = saved.model
                && model != device.model().display_name()
            {
                return Err(ProtocolError::InvalidValue(
                    "snapshot belongs to a different microphone model",
                )
                .into());
            }
            let config = saved_config(saved.config)?;
            print_json(&device.write_config(&config)?.view())?;
        }
        Command::Dial { mode } => apply(&mut device, Setting::DialMode(mode.into()))?,
        Command::Clipguard { state } => apply(&mut device, Setting::Clipguard(state.into()))?,
        Command::ClipguardMode { mode } => apply(&mut device, Setting::ClipguardMode(mode.into()))?,
        Command::Lowcut { state } => apply(&mut device, Setting::Lowcut(state.into()))?,
        Command::Gain { db } => apply(&mut device, Setting::Gain(db))?,
        Command::Mute { state } => apply(&mut device, Setting::Mute(state.into()))?,
        Command::OutputVolume { db } => apply(&mut device, Setting::OutputVolume(db))?,
        Command::MonitorMix { percent } => apply(&mut device, Setting::MonitorMix(percent))?,
        Command::Expander { state } => apply(&mut device, Setting::Expander(state.into()))?,
        Command::VoiceTune { state } => apply(&mut device, Setting::VoiceTune(state.into()))?,
        Command::VoiceTuneStrength { percent } => {
            apply(&mut device, Setting::VoiceTuneStrength(percent))?
        }
        Command::Compressor { state } => apply(&mut device, Setting::Compressor(state.into()))?,
        Command::Eq { command } => match command {
            EqCommand::Enabled { state } => apply(&mut device, Setting::EqEnabled(state.into()))?,
            EqCommand::Band {
                index,
                enabled,
                kind,
                gain_db,
                frequency_hz,
                q,
            } => apply(
                &mut device,
                Setting::EqBand {
                    index,
                    update: EqBandUpdate {
                        enabled: enabled.map(Into::into),
                        kind: kind.map(Into::into),
                        gain_db,
                        frequency_hz,
                        q,
                    },
                },
            )?,
        },
        Command::Dynamics { command } => {
            let update = match command {
                DynamicsCommand::Compressor {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    makeup_gain_db,
                } => DynamicsUpdate {
                    compressor_threshold_db: threshold_db,
                    compressor_ratio: ratio,
                    compressor_attack_ms: attack_ms,
                    compressor_release_ms: release_ms,
                    compressor_makeup_gain_db: makeup_gain_db,
                    ..DynamicsUpdate::default()
                },
                DynamicsCommand::Expander {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    max_reduction_db,
                } => DynamicsUpdate {
                    expander_threshold_db: threshold_db,
                    expander_ratio: ratio,
                    expander_attack_ms: attack_ms,
                    expander_release_ms: release_ms,
                    expander_max_reduction_db: max_reduction_db,
                    ..DynamicsUpdate::default()
                },
            };
            apply(&mut device, Setting::Dynamics(update))?;
        }
    }
    Ok(())
}

fn apply(device: &mut WaveDevice<UsbTransport>, setting: Setting) -> Result<(), CliError> {
    print_json(&device.apply(setting)?.view())?;
    Ok(())
}

fn state(device: &mut WaveDevice<UsbTransport>) -> Result<State, DeviceError> {
    let model = device.model().display_name().to_owned();
    let info = device.read_info()?.view();
    let config = device.read_config()?.view();
    let meter_hex = device.read_meter()?.map(|meter| format_hex(&meter));
    Ok(State {
        device: info,
        model,
        config,
        meter_hex,
    })
}

fn saved_config(saved: SavedConfig) -> Result<DeviceConfig, ProtocolError> {
    if let Some(raw) = saved.raw_hex {
        return Config::from_hex(&raw).map(DeviceConfig::Block16);
    }
    let (Some(settings), Some(headphones), Some(monitor)) =
        (saved.settings_hex, saved.headphones_hex, saved.monitor_hex)
    else {
        return Err(ProtocolError::InvalidValue(
            "snapshot must contain a block16 config or all three Wave:3 MK.2 blocks",
        ));
    };
    let settings = decode_hex(
        &settings,
        crate::wave3_mk2::SETTINGS_LENGTH,
        "Wave:3 MK.2 settings",
    )?;
    let headphones = decode_hex(
        &headphones,
        crate::wave3_mk2::HEADPHONES_LENGTH,
        "Wave:3 MK.2 headphones",
    )?;
    let monitor = decode_hex(
        &monitor,
        crate::wave3_mk2::MONITOR_LENGTH,
        "Wave:3 MK.2 monitor",
    )?;
    Wave3Mk2Config::from_blocks(&settings, &headphones, &monitor).map(DeviceConfig::Wave3Mk2)
}

fn print_json(value: &impl Serialize) -> Result<(), serde_json::Error> {
    print!("{}", json_text(value)?);
    Ok(())
}

fn json_text(value: &impl Serialize) -> Result<String, serde_json::Error> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_both_snapshot_formats() {
        let block16 = saved_config(SavedConfig {
            raw_hex: Some(format_hex(&[0x5a; 16])),
            settings_hex: None,
            headphones_hex: None,
            monitor_hex: None,
        })
        .unwrap();
        assert!(matches!(
            block16,
            DeviceConfig::Block16(ref config) if config.raw() == &[0x5a; 16]
        ));

        let mk2 = saved_config(SavedConfig {
            raw_hex: None,
            settings_hex: Some(format_hex(&[0x22; 38])),
            headphones_hex: Some(format_hex(&[0x30, 0])),
            monitor_hex: Some(format_hex(&[0x32, 1, 2, 3, 2, 0])),
        })
        .unwrap();
        assert!(matches!(
            mk2,
            DeviceConfig::Wave3Mk2(ref config)
                if config.settings_raw() == &[0x22; 38]
                    && config.headphones_raw() == &[0x30, 0]
                    && config.monitor_raw() == &[0x32, 1, 2, 3, 2, 0]
        ));
    }
}
