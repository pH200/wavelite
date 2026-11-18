use crate::{
    controller::Event,
    device::{
        DeviceConfig as CoreDeviceConfig, DeviceConfigView, DeviceModel as CoreDeviceModel,
        Setting as CoreSetting,
    },
    protocol::{
        ConfigView, DeviceInfo as CoreDeviceInfo, DeviceInfoView, DialMode as CoreDialMode,
    },
    wave3_mk2::{
        ClipguardMode as CoreClipguardMode, CompressorView as CoreCompressorView,
        DynamicsUpdate as CoreDynamicsUpdate, EqBandType as CoreEqBandType,
        EqBandUpdate as CoreEqBandUpdate, EqBandView as CoreEqBandView, EqView as CoreEqView,
        ExpanderView as CoreExpanderView, Wave3Mk2ConfigView,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DeviceModel {
    Wave1,
    Wave3,
    Wave3Mk2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DemoModel {
    Wave3,
    Wave3Mk2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ClipguardMode {
    Enabled,
    Bypassed,
    HardClip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DialMode {
    InputGain,
    OutputVolume,
    MonitorMix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EqBandType {
    HighPass,
    LowShelf,
    Bell,
    Notch,
    HighShelf,
    LowPass,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, uniffi::Record)]
pub struct EqBandUpdate {
    #[uniffi(default)]
    pub enabled: Option<bool>,
    #[uniffi(default)]
    pub kind: Option<EqBandType>,
    #[uniffi(default)]
    pub gain_db: Option<f64>,
    #[uniffi(default)]
    pub frequency_hz: Option<f64>,
    #[uniffi(default)]
    pub q: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, uniffi::Record)]
pub struct DynamicsUpdate {
    #[uniffi(default)]
    pub compressor_threshold_db: Option<f64>,
    #[uniffi(default)]
    pub compressor_ratio: Option<f64>,
    #[uniffi(default)]
    pub compressor_attack_ms: Option<f64>,
    #[uniffi(default)]
    pub compressor_release_ms: Option<f64>,
    #[uniffi(default)]
    pub compressor_makeup_gain_db: Option<f64>,
    #[uniffi(default)]
    pub expander_threshold_db: Option<f64>,
    #[uniffi(default)]
    pub expander_ratio: Option<f64>,
    #[uniffi(default)]
    pub expander_attack_ms: Option<f64>,
    #[uniffi(default)]
    pub expander_release_ms: Option<f64>,
    #[uniffi(default)]
    pub expander_max_reduction_db: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Setting {
    Gain(f64),
    Mute(bool),
    Clipguard(bool),
    ClipguardMode(ClipguardMode),
    Lowcut(bool),
    OutputVolume(f64),
    MonitorMix(f64),
    DialMode(DialMode),
    Expander(bool),
    VoiceTune(bool),
    VoiceTuneStrength(f64),
    Compressor(bool),
    EqEnabled(bool),
    EqBand { index: u8, update: EqBandUpdate },
    Dynamics(DynamicsUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DeviceInfo {
    pub api_version: String,
    pub firmware_version: String,
    pub serial: String,
    pub raw_hex: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EqBandView {
    pub enabled: bool,
    pub kind: Option<EqBandType>,
    pub gain_db: f64,
    pub frequency_hz: u16,
    pub q: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EqView {
    pub enabled: bool,
    pub bands: Vec<EqBandView>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CompressorView {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: u16,
    pub release_ms: u16,
    pub makeup_gain_db: u8,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ExpanderView {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: u16,
    pub release_ms: u16,
    pub max_reduction_db: u8,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DeviceConfig {
    pub raw_hex: Option<String>,
    pub settings_hex: Option<String>,
    pub headphones_hex: Option<String>,
    pub monitor_hex: Option<String>,
    pub gain_db: f64,
    pub muted: bool,
    pub clipguard: bool,
    pub clipguard_mode: Option<ClipguardMode>,
    pub lowcut: bool,
    pub expander: Option<bool>,
    pub voice_tune: Option<bool>,
    pub voice_tune_strength_percent: Option<u8>,
    pub compressor: Option<bool>,
    pub output_volume_db: f64,
    pub monitor_mix_percent: f64,
    pub dial_mode: Option<DialMode>,
    pub eq: Option<EqView>,
    pub compressor_settings: Option<CompressorView>,
    pub expander_settings: Option<ExpanderView>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum DeviceEvent {
    Connected {
        model: DeviceModel,
        display_name: String,
        info: DeviceInfo,
        config: DeviceConfig,
    },
    ConfigChanged(DeviceConfig),
    Refreshed(DeviceConfig),
    Disconnected(String),
    OperationFailed(String),
}

impl From<DemoModel> for CoreDeviceModel {
    fn from(model: DemoModel) -> Self {
        match model {
            DemoModel::Wave3 => Self::Wave3,
            DemoModel::Wave3Mk2 => Self::Wave3Mk2,
        }
    }
}

impl From<CoreDeviceModel> for DeviceModel {
    fn from(model: CoreDeviceModel) -> Self {
        match model {
            CoreDeviceModel::Wave1 => Self::Wave1,
            CoreDeviceModel::Wave3 => Self::Wave3,
            CoreDeviceModel::Wave3Mk2 => Self::Wave3Mk2,
        }
    }
}

impl From<ClipguardMode> for CoreClipguardMode {
    fn from(mode: ClipguardMode) -> Self {
        match mode {
            ClipguardMode::Enabled => Self::Enabled,
            ClipguardMode::Bypassed => Self::Bypassed,
            ClipguardMode::HardClip => Self::HardClip,
        }
    }
}

impl From<DialMode> for CoreDialMode {
    fn from(mode: DialMode) -> Self {
        match mode {
            DialMode::InputGain => Self::InputGain,
            DialMode::OutputVolume => Self::OutputVolume,
            DialMode::MonitorMix => Self::MonitorMix,
        }
    }
}

impl From<EqBandType> for CoreEqBandType {
    fn from(kind: EqBandType) -> Self {
        match kind {
            EqBandType::HighPass => Self::HighPass,
            EqBandType::LowShelf => Self::LowShelf,
            EqBandType::Bell => Self::Bell,
            EqBandType::Notch => Self::Notch,
            EqBandType::HighShelf => Self::HighShelf,
            EqBandType::LowPass => Self::LowPass,
        }
    }
}

impl From<EqBandUpdate> for CoreEqBandUpdate {
    fn from(update: EqBandUpdate) -> Self {
        Self {
            enabled: update.enabled,
            kind: update.kind.map(Into::into),
            gain_db: update.gain_db,
            frequency_hz: update.frequency_hz,
            q: update.q,
        }
    }
}

impl From<DynamicsUpdate> for CoreDynamicsUpdate {
    fn from(update: DynamicsUpdate) -> Self {
        Self {
            compressor_threshold_db: update.compressor_threshold_db,
            compressor_ratio: update.compressor_ratio,
            compressor_attack_ms: update.compressor_attack_ms,
            compressor_release_ms: update.compressor_release_ms,
            compressor_makeup_gain_db: update.compressor_makeup_gain_db,
            expander_threshold_db: update.expander_threshold_db,
            expander_ratio: update.expander_ratio,
            expander_attack_ms: update.expander_attack_ms,
            expander_release_ms: update.expander_release_ms,
            expander_max_reduction_db: update.expander_max_reduction_db,
        }
    }
}

impl From<Setting> for CoreSetting {
    fn from(setting: Setting) -> Self {
        match setting {
            Setting::Gain(value) => Self::Gain(value),
            Setting::Mute(value) => Self::Mute(value),
            Setting::Clipguard(value) => Self::Clipguard(value),
            Setting::ClipguardMode(value) => Self::ClipguardMode(value.into()),
            Setting::Lowcut(value) => Self::Lowcut(value),
            Setting::OutputVolume(value) => Self::OutputVolume(value),
            Setting::MonitorMix(value) => Self::MonitorMix(value),
            Setting::DialMode(value) => Self::DialMode(value.into()),
            Setting::Expander(value) => Self::Expander(value),
            Setting::VoiceTune(value) => Self::VoiceTune(value),
            Setting::VoiceTuneStrength(value) => Self::VoiceTuneStrength(value),
            Setting::Compressor(value) => Self::Compressor(value),
            Setting::EqEnabled(value) => Self::EqEnabled(value),
            Setting::EqBand { index, update } => Self::EqBand {
                index: usize::from(index),
                update: update.into(),
            },
            Setting::Dynamics(update) => Self::Dynamics(update.into()),
        }
    }
}

impl From<CoreDeviceInfo> for DeviceInfo {
    fn from(info: CoreDeviceInfo) -> Self {
        info.view().into()
    }
}

impl From<DeviceInfoView> for DeviceInfo {
    fn from(info: DeviceInfoView) -> Self {
        Self {
            api_version: info.api_version,
            firmware_version: info.firmware_version,
            serial: info.serial,
            raw_hex: info.raw_hex,
        }
    }
}

impl From<CoreEqBandView> for EqBandView {
    fn from(view: CoreEqBandView) -> Self {
        Self {
            enabled: view.enabled,
            kind: parse_eq_kind(&view.kind),
            gain_db: view.gain_db,
            frequency_hz: view.frequency_hz,
            q: view.q,
        }
    }
}

impl From<CoreEqView> for EqView {
    fn from(view: CoreEqView) -> Self {
        Self {
            enabled: view.enabled,
            bands: view.bands.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CoreCompressorView> for CompressorView {
    fn from(view: CoreCompressorView) -> Self {
        Self {
            threshold_db: view.threshold_db,
            ratio: view.ratio,
            attack_ms: view.attack_ms,
            release_ms: view.release_ms,
            makeup_gain_db: view.makeup_gain_db,
        }
    }
}

impl From<CoreExpanderView> for ExpanderView {
    fn from(view: CoreExpanderView) -> Self {
        Self {
            threshold_db: view.threshold_db,
            ratio: view.ratio,
            attack_ms: view.attack_ms,
            release_ms: view.release_ms,
            max_reduction_db: view.max_reduction_db,
        }
    }
}

impl From<CoreDeviceConfig> for DeviceConfig {
    fn from(config: CoreDeviceConfig) -> Self {
        config.view().into()
    }
}

impl From<DeviceConfigView> for DeviceConfig {
    fn from(config: DeviceConfigView) -> Self {
        match config {
            DeviceConfigView::Block16(view) => view.into(),
            DeviceConfigView::Wave3Mk2(view) => (*view).into(),
        }
    }
}

impl From<ConfigView> for DeviceConfig {
    fn from(view: ConfigView) -> Self {
        Self {
            raw_hex: Some(view.raw_hex),
            settings_hex: None,
            headphones_hex: None,
            monitor_hex: None,
            gain_db: view.gain_db,
            muted: view.muted,
            clipguard: view.clipguard,
            clipguard_mode: None,
            lowcut: view.lowcut,
            expander: None,
            voice_tune: None,
            voice_tune_strength_percent: None,
            compressor: None,
            output_volume_db: view.output_volume_db,
            monitor_mix_percent: view.monitor_mix_percent,
            dial_mode: parse_dial_mode(&view.dial_mode),
            eq: None,
            compressor_settings: None,
            expander_settings: None,
        }
    }
}

impl From<Wave3Mk2ConfigView> for DeviceConfig {
    fn from(view: Wave3Mk2ConfigView) -> Self {
        Self {
            raw_hex: None,
            settings_hex: Some(view.settings_hex),
            headphones_hex: Some(view.headphones_hex),
            monitor_hex: Some(view.monitor_hex),
            gain_db: view.gain_db,
            muted: view.muted,
            clipguard: view.clipguard,
            clipguard_mode: parse_clipguard_mode(&view.clipguard_mode),
            lowcut: view.lowcut,
            expander: Some(view.expander),
            voice_tune: Some(view.voice_tune),
            voice_tune_strength_percent: Some(view.voice_tune_strength_percent),
            compressor: Some(view.compressor),
            output_volume_db: view.output_volume_db,
            monitor_mix_percent: view.monitor_mix_percent,
            dial_mode: None,
            eq: Some(view.eq.into()),
            compressor_settings: Some(view.compressor_settings.into()),
            expander_settings: Some(view.expander_settings.into()),
        }
    }
}

impl From<Event> for DeviceEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::Connected {
                model,
                info,
                config,
            } => Self::Connected {
                model: model.into(),
                display_name: CoreDeviceModel::display_name(model).to_owned(),
                info: info.into(),
                config: config.into(),
            },
            Event::ConfigChanged(config) => Self::ConfigChanged(config.into()),
            Event::Refreshed(config) => Self::Refreshed(config.into()),
            Event::Disconnected(message) => Self::Disconnected(message),
            Event::OperationFailed(message) => Self::OperationFailed(message),
        }
    }
}

fn parse_clipguard_mode(value: &str) -> Option<ClipguardMode> {
    match value {
        "enabled" => Some(ClipguardMode::Enabled),
        "bypassed" => Some(ClipguardMode::Bypassed),
        "hard_clip" => Some(ClipguardMode::HardClip),
        _ => None,
    }
}

fn parse_dial_mode(value: &str) -> Option<DialMode> {
    match value {
        "input_gain" => Some(DialMode::InputGain),
        "output_volume" => Some(DialMode::OutputVolume),
        "monitor_mix" => Some(DialMode::MonitorMix),
        _ => None,
    }
}

fn parse_eq_kind(value: &str) -> Option<EqBandType> {
    match value {
        "high_pass" => Some(EqBandType::HighPass),
        "low_shelf" => Some(EqBandType::LowShelf),
        "bell" => Some(EqBandType::Bell),
        "notch" => Some(EqBandType::Notch),
        "high_shelf" => Some(EqBandType::HighShelf),
        "low_pass" => Some(EqBandType::LowPass),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_cover_common_dial_dsp_dynamics_and_eq() {
        let cases = [
            (Setting::Gain(12.5), CoreSetting::Gain(12.5)),
            (Setting::Mute(true), CoreSetting::Mute(true)),
            (Setting::Clipguard(false), CoreSetting::Clipguard(false)),
            (
                Setting::ClipguardMode(ClipguardMode::HardClip),
                CoreSetting::ClipguardMode(CoreClipguardMode::HardClip),
            ),
            (Setting::Lowcut(false), CoreSetting::Lowcut(false)),
            (
                Setting::OutputVolume(-20.0),
                CoreSetting::OutputVolume(-20.0),
            ),
            (Setting::MonitorMix(25.0), CoreSetting::MonitorMix(25.0)),
            (
                Setting::DialMode(DialMode::MonitorMix),
                CoreSetting::DialMode(CoreDialMode::MonitorMix),
            ),
            (Setting::Expander(true), CoreSetting::Expander(true)),
            (Setting::VoiceTune(true), CoreSetting::VoiceTune(true)),
            (
                Setting::VoiceTuneStrength(50.0),
                CoreSetting::VoiceTuneStrength(50.0),
            ),
            (Setting::Compressor(true), CoreSetting::Compressor(true)),
            (Setting::EqEnabled(true), CoreSetting::EqEnabled(true)),
            (
                Setting::EqBand {
                    index: 2,
                    update: EqBandUpdate {
                        enabled: Some(true),
                        kind: Some(EqBandType::Bell),
                        gain_db: Some(3.0),
                        ..EqBandUpdate::default()
                    },
                },
                CoreSetting::EqBand {
                    index: 2,
                    update: CoreEqBandUpdate {
                        enabled: Some(true),
                        kind: Some(CoreEqBandType::Bell),
                        gain_db: Some(3.0),
                        ..CoreEqBandUpdate::default()
                    },
                },
            ),
            (
                Setting::Dynamics(DynamicsUpdate {
                    compressor_threshold_db: Some(-20.0),
                    expander_release_ms: Some(200.0),
                    ..DynamicsUpdate::default()
                }),
                CoreSetting::Dynamics(CoreDynamicsUpdate {
                    compressor_threshold_db: Some(-20.0),
                    expander_release_ms: Some(200.0),
                    ..CoreDynamicsUpdate::default()
                }),
            ),
        ];

        for (bridge, expected) in cases {
            assert_eq!(CoreSetting::from(bridge), expected, "{bridge:?}");
        }
    }

    #[test]
    fn unknown_view_modes_are_absent() {
        assert_eq!(parse_clipguard_mode("unknown_9"), None);
        assert_eq!(parse_dial_mode("unknown_9"), None);
        assert_eq!(parse_eq_kind("unknown_9"), None);
    }
}
