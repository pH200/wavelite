use std::time::Duration;

use rusb::{Context, Device, DeviceHandle, UsbContext};
use serde::Serialize;
use thiserror::Error;

use crate::{
    protocol::{
        CONFIG_LENGTH, Config, ConfigView, DeviceInfo, DialMode, INFO_LENGTH, METER_LENGTH,
        ProtocolError, format_hex,
    },
    wave3_mk2::{
        self, ClipguardMode, DynamicsUpdate, EqBandUpdate, HEADPHONES_VALUE,
        INFO_VALUE as WAVE3_MK2_INFO_VALUE, MONITOR_VALUE, SETTINGS_VALUE, Wave3Mk2Config,
        Wave3Mk2ConfigView,
    },
};

pub const ELGATO_VENDOR_ID: u16 = 0x0fd9;
pub const WAVE1_VENDOR_ID: u16 = ELGATO_VENDOR_ID;
pub const WAVE1_PRODUCT_ID: u16 = 0x006e;
pub const WAVE3_PRODUCT_ID: u16 = 0x0070;
pub const WAVE3_MK2_PRODUCT_ID: u16 = 0x00ca;
pub const REQUEST_INDEX: u16 = 0x3303;
pub const WAVE3_MK2_REQUEST_INDEX: u16 = 0x0103;
pub const READ_REQUEST: u8 = 0x85;
pub const WRITE_REQUEST: u8 = 0x05;
pub const WAVE3_MK2_REQUEST: u8 = 0x01;
pub const CONFIG_VALUE: u16 = 0;
pub const METER_VALUE: u16 = 1;
pub const INFO_VALUE: u16 = 0x000a;

const TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceModel {
    Wave1,
    Wave3,
    Wave3Mk2,
}

impl DeviceModel {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wave1 => "Elgato Wave:1",
            Self::Wave3 => "Elgato Wave:3 MK.1",
            Self::Wave3Mk2 => "Elgato Wave:3 MK.2",
        }
    }

    pub const fn product_id(self) -> u16 {
        match self {
            Self::Wave1 => WAVE1_PRODUCT_ID,
            Self::Wave3 => WAVE3_PRODUCT_ID,
            Self::Wave3Mk2 => WAVE3_MK2_PRODUCT_ID,
        }
    }

    pub fn from_product_id(product_id: u16) -> Option<Self> {
        match product_id {
            WAVE1_PRODUCT_ID => Some(Self::Wave1),
            WAVE3_PRODUCT_ID => Some(Self::Wave3),
            WAVE3_MK2_PRODUCT_ID => Some(Self::Wave3Mk2),
            _ => None,
        }
    }

    pub const fn is_wave3_mk2(self) -> bool {
        matches!(self, Self::Wave3Mk2)
    }
}

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("USB discovery failed: {0}")]
    Discovery(#[source] rusb::Error),
    #[error("no supported Elgato Wave microphone found")]
    NotFound,
    #[error("multiple supported Wave devices found; specify --serial")]
    MultipleDevices,
    #[error("cannot read Wave serial number: {0}")]
    Serial(#[source] rusb::Error),
    #[error("cannot open Wave microphone: {0}")]
    Open(#[source] rusb::Error),
    #[error("USB read failed: {0}")]
    Read(#[source] rusb::Error),
    #[error("USB write failed: {0}")]
    Write(#[source] rusb::Error),
    #[error("short USB read: expected {expected} bytes, got {actual}")]
    ShortRead { expected: usize, actual: usize },
    #[error("short USB write: expected {expected} bytes, wrote {actual}")]
    ShortWrite { expected: usize, actual: usize },
    #[error("config readback mismatch: expected {expected}, got {actual}")]
    ReadbackMismatch { expected: String, actual: String },
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

pub trait Transport {
    fn control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError>;
    fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError>;

    fn wave3_mk2_control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
        let _ = (value, length);
        Err(unsupported("Wave:3 MK.2 control reads", self.model()))
    }

    fn wave3_mk2_control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        let _ = (value, raw);
        Err(unsupported("Wave:3 MK.2 control writes", self.model()))
    }

    fn model(&self) -> DeviceModel {
        DeviceModel::Wave1
    }
}

pub struct UsbTransport {
    device: DeviceHandle<Context>,
    model: DeviceModel,
}

impl UsbTransport {
    pub fn open(serial: Option<&str>) -> Result<Self, DeviceError> {
        Self::open_matching(serial, |_| true)
    }

    pub fn open_model(serial: Option<&str>, model: DeviceModel) -> Result<Self, DeviceError> {
        Self::open_matching(serial, |candidate| candidate == model)
    }

    fn open_matching(
        serial: Option<&str>,
        wanted: impl Fn(DeviceModel) -> bool,
    ) -> Result<Self, DeviceError> {
        let context = Context::new().map_err(DeviceError::Discovery)?;
        let devices = context.devices().map_err(DeviceError::Discovery)?;
        let mut matched: Vec<(Device<Context>, DeviceModel)> = Vec::new();

        for device in devices.iter() {
            let descriptor = device.device_descriptor().map_err(DeviceError::Discovery)?;
            if descriptor.vendor_id() != ELGATO_VENDOR_ID {
                continue;
            }
            let Some(model) = DeviceModel::from_product_id(descriptor.product_id()) else {
                continue;
            };
            if !wanted(model) {
                continue;
            }
            if let Some(expected) = serial {
                let handle = device.open().map_err(DeviceError::Serial)?;
                let actual = handle
                    .read_serial_number_string_ascii(&descriptor)
                    .map_err(DeviceError::Serial)?;
                if actual != expected {
                    continue;
                }
            }
            matched.push((device, model));
        }

        match matched.len() {
            0 => Err(DeviceError::NotFound),
            1 => {
                let (device, model) = matched.remove(0);
                let device = device.open().map_err(DeviceError::Open)?;
                if model.is_wave3_mk2() {
                    device.claim_interface(3).map_err(DeviceError::Open)?;
                }
                Ok(Self { device, model })
            }
            _ => Err(DeviceError::MultipleDevices),
        }
    }
}

impl Transport for UsbTransport {
    fn control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
        let mut raw = vec![0; length];
        let actual = self
            .device
            .read_control(0xa1, READ_REQUEST, value, REQUEST_INDEX, &mut raw, TIMEOUT)
            .map_err(DeviceError::Read)?;
        exact_read(length, actual, raw)
    }

    fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        let actual = self
            .device
            .write_control(0x21, WRITE_REQUEST, value, REQUEST_INDEX, raw, TIMEOUT)
            .map_err(DeviceError::Write)?;
        exact_write(raw.len(), actual)
    }

    fn wave3_mk2_control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
        let mut raw = vec![0; length];
        let actual = self
            .device
            .read_control(
                0xc1,
                WAVE3_MK2_REQUEST,
                value,
                WAVE3_MK2_REQUEST_INDEX,
                &mut raw,
                TIMEOUT,
            )
            .map_err(DeviceError::Read)?;
        exact_read(length, actual, raw)
    }

    fn wave3_mk2_control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        let actual = self
            .device
            .write_control(
                0x41,
                WAVE3_MK2_REQUEST,
                value,
                WAVE3_MK2_REQUEST_INDEX,
                raw,
                TIMEOUT,
            )
            .map_err(DeviceError::Write)?;
        exact_write(raw.len(), actual)
    }

    fn model(&self) -> DeviceModel {
        self.model
    }
}

fn exact_read(expected: usize, actual: usize, raw: Vec<u8>) -> Result<Vec<u8>, DeviceError> {
    if actual != expected {
        return Err(DeviceError::ShortRead { expected, actual });
    }
    Ok(raw)
}

fn exact_write(expected: usize, actual: usize) -> Result<(), DeviceError> {
    if actual != expected {
        return Err(DeviceError::ShortWrite { expected, actual });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceConfig {
    Block16(Config),
    Wave3Mk2(Wave3Mk2Config),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum DeviceConfigView {
    Block16(ConfigView),
    Wave3Mk2(Box<Wave3Mk2ConfigView>),
}

impl DeviceConfig {
    pub fn view(&self) -> DeviceConfigView {
        match self {
            Self::Block16(config) => DeviceConfigView::Block16(config.view()),
            Self::Wave3Mk2(config) => DeviceConfigView::Wave3Mk2(Box::new(config.view())),
        }
    }

    pub fn gain_db(&self) -> f64 {
        match self {
            Self::Block16(config) => config.gain_db(),
            Self::Wave3Mk2(config) => config.gain_db(),
        }
    }

    pub fn muted(&self) -> bool {
        match self {
            Self::Block16(config) => config.muted(),
            Self::Wave3Mk2(config) => config.muted(),
        }
    }

    pub fn clipguard(&self) -> bool {
        match self {
            Self::Block16(config) => config.clipguard(),
            Self::Wave3Mk2(config) => config.clipguard(),
        }
    }

    pub fn lowcut(&self) -> bool {
        match self {
            Self::Block16(config) => config.lowcut(),
            Self::Wave3Mk2(config) => config.lowcut(),
        }
    }

    pub fn output_volume_db(&self) -> f64 {
        match self {
            Self::Block16(config) => config.output_volume_db(),
            Self::Wave3Mk2(config) => config.output_volume_db(),
        }
    }

    pub fn monitor_mix_percent(&self) -> f64 {
        match self {
            Self::Block16(config) => config.monitor_mix_percent(),
            Self::Wave3Mk2(config) => config.monitor_mix_percent(),
        }
    }

    pub fn block16(&self) -> Option<&Config> {
        match self {
            Self::Block16(config) => Some(config),
            Self::Wave3Mk2(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
    EqBand { index: usize, update: EqBandUpdate },
    Dynamics(DynamicsUpdate),
}

impl Setting {
    fn feature(self) -> &'static str {
        match self {
            Self::Gain(_) => "input gain",
            Self::Mute(_) => "mute",
            Self::Clipguard(_) | Self::ClipguardMode(_) => "Clipguard",
            Self::Lowcut(_) => "lowcut",
            Self::OutputVolume(_) => "output volume",
            Self::MonitorMix(_) => "monitor mix",
            Self::DialMode(_) => "dial function",
            Self::Expander(_) => "expander",
            Self::VoiceTune(_) | Self::VoiceTuneStrength(_) => "voice tune",
            Self::Compressor(_) => "compressor",
            Self::EqEnabled(_) | Self::EqBand { .. } => "hardware EQ",
            Self::Dynamics(_) => "hardware dynamics",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceSpec {
    pub model: DeviceModel,
    pub dial_modes: &'static [DialMode],
    pub clipguard_modes: bool,
    pub expander: bool,
    pub voice_tune: bool,
    pub compressor: bool,
    pub hardware_eq: bool,
    pub hardware_dynamics: bool,
}

pub static WAVE1_SPEC: DeviceSpec = DeviceSpec {
    model: DeviceModel::Wave1,
    dial_modes: &[DialMode::InputGain, DialMode::OutputVolume],
    clipguard_modes: false,
    expander: false,
    voice_tune: false,
    compressor: false,
    hardware_eq: false,
    hardware_dynamics: false,
};

pub static WAVE3_SPEC: DeviceSpec = DeviceSpec {
    model: DeviceModel::Wave3,
    dial_modes: &[
        DialMode::InputGain,
        DialMode::OutputVolume,
        DialMode::MonitorMix,
    ],
    clipguard_modes: false,
    expander: false,
    voice_tune: false,
    compressor: false,
    hardware_eq: false,
    hardware_dynamics: false,
};

pub static WAVE3_MK2_SPEC: DeviceSpec = DeviceSpec {
    model: DeviceModel::Wave3Mk2,
    dial_modes: &[],
    clipguard_modes: true,
    expander: true,
    voice_tune: true,
    compressor: true,
    hardware_eq: true,
    hardware_dynamics: true,
};

pub trait WaveDriver<T: Transport> {
    fn spec(&self) -> &'static DeviceSpec;
    fn transport(&self) -> &T;
    fn read_info(&mut self) -> Result<DeviceInfo, DeviceError>;
    fn read_config(&mut self) -> Result<DeviceConfig, DeviceError>;
    fn write_config(&mut self, config: &DeviceConfig) -> Result<DeviceConfig, DeviceError>;
    fn set_gain(&mut self, db: f64) -> Result<DeviceConfig, DeviceError>;
    fn set_mute(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError>;
    fn set_clipguard(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError>;
    fn set_lowcut(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError>;
    fn set_output_volume(&mut self, db: f64) -> Result<DeviceConfig, DeviceError>;
    fn set_monitor_mix(&mut self, percent: f64) -> Result<DeviceConfig, DeviceError>;

    fn read_meter(&mut self) -> Result<Option<Vec<u8>>, DeviceError> {
        Ok(None)
    }

    fn set_dial_mode(&mut self, mode: DialMode) -> Result<DeviceConfig, DeviceError> {
        let _ = mode;
        Err(unsupported("dial function", self.spec().model))
    }

    fn set_clipguard_mode(&mut self, mode: ClipguardMode) -> Result<DeviceConfig, DeviceError> {
        let _ = mode;
        Err(unsupported("Clipguard modes", self.spec().model))
    }

    fn set_expander(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        let _ = enabled;
        Err(unsupported("expander", self.spec().model))
    }

    fn set_voice_tune(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        let _ = enabled;
        Err(unsupported("voice tune", self.spec().model))
    }

    fn set_voice_tune_strength(&mut self, percent: f64) -> Result<DeviceConfig, DeviceError> {
        let _ = percent;
        Err(unsupported("voice tune strength", self.spec().model))
    }

    fn set_compressor(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        let _ = enabled;
        Err(unsupported("compressor", self.spec().model))
    }

    fn set_eq_enabled(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        let _ = enabled;
        Err(unsupported("hardware EQ", self.spec().model))
    }

    fn set_eq_band(
        &mut self,
        index: usize,
        update: EqBandUpdate,
    ) -> Result<DeviceConfig, DeviceError> {
        let _ = (index, update);
        Err(unsupported("hardware EQ bands", self.spec().model))
    }

    fn set_dynamics(&mut self, update: DynamicsUpdate) -> Result<DeviceConfig, DeviceError> {
        let _ = update;
        Err(unsupported("hardware dynamics", self.spec().model))
    }

    fn apply(&mut self, setting: Setting) -> Result<DeviceConfig, DeviceError> {
        self.require_supported(setting)?;
        match setting {
            Setting::Gain(value) => self.set_gain(value),
            Setting::Mute(value) => self.set_mute(value),
            Setting::Clipguard(value) => self.set_clipguard(value),
            Setting::ClipguardMode(value) => self.set_clipguard_mode(value),
            Setting::Lowcut(value) => self.set_lowcut(value),
            Setting::OutputVolume(value) => self.set_output_volume(value),
            Setting::MonitorMix(value) => self.set_monitor_mix(value),
            Setting::DialMode(value) => self.set_dial_mode(value),
            Setting::Expander(value) => self.set_expander(value),
            Setting::VoiceTune(value) => self.set_voice_tune(value),
            Setting::VoiceTuneStrength(value) => self.set_voice_tune_strength(value),
            Setting::Compressor(value) => self.set_compressor(value),
            Setting::EqEnabled(value) => self.set_eq_enabled(value),
            Setting::EqBand { index, update } => self.set_eq_band(index, update),
            Setting::Dynamics(update) => self.set_dynamics(update),
        }
    }

    fn require_supported(&self, setting: Setting) -> Result<(), DeviceError> {
        let spec = self.spec();
        let supported = match setting {
            Setting::ClipguardMode(_) => spec.clipguard_modes,
            Setting::DialMode(mode) => spec.dial_modes.contains(&mode),
            Setting::Expander(_) => spec.expander,
            Setting::VoiceTune(_) | Setting::VoiceTuneStrength(_) => spec.voice_tune,
            Setting::Compressor(_) => spec.compressor,
            Setting::EqEnabled(_) | Setting::EqBand { .. } => spec.hardware_eq,
            Setting::Dynamics(_) => spec.hardware_dynamics,
            _ => true,
        };
        if supported {
            Ok(())
        } else {
            Err(unsupported(setting.feature(), spec.model))
        }
    }
}

pub struct Q88Device<T> {
    transport: T,
    spec: &'static DeviceSpec,
}

pub type Wave1Device<T> = Q88Device<T>;
pub type Wave3Device<T> = Q88Device<T>;

impl<T: Transport> Q88Device<T> {
    pub fn new(transport: T) -> Self {
        let spec = match transport.model() {
            DeviceModel::Wave1 => &WAVE1_SPEC,
            DeviceModel::Wave3 => &WAVE3_SPEC,
            DeviceModel::Wave3Mk2 => panic!("Wave:3 MK.2 requires its vendor-block driver"),
        };
        Self { transport, spec }
    }

    pub fn model(&self) -> DeviceModel {
        self.spec.model
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn read_config(&mut self) -> Result<Config, DeviceError> {
        let raw = self.transport.control_in(CONFIG_VALUE, CONFIG_LENGTH)?;
        Config::from_bytes(&raw).map_err(DeviceError::from)
    }

    pub fn read_info(&mut self) -> Result<DeviceInfo, DeviceError> {
        let raw = self.transport.control_in(INFO_VALUE, INFO_LENGTH)?;
        DeviceInfo::from_bytes(&raw).map_err(DeviceError::from)
    }

    pub fn read_meter(&mut self) -> Result<Vec<u8>, DeviceError> {
        self.transport.control_in(METER_VALUE, METER_LENGTH)
    }

    pub fn write_config(&mut self, config: &Config) -> Result<Config, DeviceError> {
        self.transport.control_out(CONFIG_VALUE, config.raw())?;
        let readback = self.read_config()?;
        if readback != *config {
            return Err(DeviceError::ReadbackMismatch {
                expected: format_hex(config.raw()),
                actual: format_hex(readback.raw()),
            });
        }
        Ok(readback)
    }

    pub fn set_dial_mode(&mut self, mode: DialMode) -> Result<Config, DeviceError> {
        if !self.spec.dial_modes.contains(&mode) {
            return Err(unsupported("dial function", self.model()));
        }
        self.update_config(|config| Ok(config.with_dial_mode(mode)))
    }

    pub fn set_clipguard(&mut self, enabled: bool) -> Result<Config, DeviceError> {
        self.update_config(|config| Ok(config.with_clipguard(enabled)))
    }

    pub fn set_lowcut(&mut self, enabled: bool) -> Result<Config, DeviceError> {
        self.update_config(|config| Ok(config.with_lowcut(enabled)))
    }

    pub fn set_gain(&mut self, db: f64) -> Result<Config, DeviceError> {
        self.update_config(|config| config.with_gain(db))
    }

    pub fn set_mute(&mut self, enabled: bool) -> Result<Config, DeviceError> {
        self.update_config(|config| Ok(config.with_mute(enabled)))
    }

    pub fn set_output_volume(&mut self, db: f64) -> Result<Config, DeviceError> {
        self.update_config(|config| config.with_output_volume(db))
    }

    pub fn set_monitor_mix(&mut self, percent: f64) -> Result<Config, DeviceError> {
        self.update_config(|config| config.with_monitor_mix(percent))
    }

    fn update_config(
        &mut self,
        update: impl FnOnce(&Config) -> Result<Config, ProtocolError>,
    ) -> Result<Config, DeviceError> {
        let current = self.read_config()?;
        let updated = update(&current)?;
        if updated == current {
            return Ok(current);
        }
        self.write_config(&updated)
    }
}

impl<T: Transport> WaveDriver<T> for Q88Device<T> {
    fn spec(&self) -> &'static DeviceSpec {
        self.spec
    }

    fn transport(&self) -> &T {
        &self.transport
    }

    fn read_info(&mut self) -> Result<DeviceInfo, DeviceError> {
        Q88Device::read_info(self)
    }

    fn read_config(&mut self) -> Result<DeviceConfig, DeviceError> {
        Q88Device::read_config(self).map(DeviceConfig::Block16)
    }

    fn read_meter(&mut self) -> Result<Option<Vec<u8>>, DeviceError> {
        Q88Device::read_meter(self).map(Some)
    }

    fn write_config(&mut self, config: &DeviceConfig) -> Result<DeviceConfig, DeviceError> {
        let DeviceConfig::Block16(config) = config else {
            return Err(unsupported(
                "restoring another driver's configuration",
                self.model(),
            ));
        };
        Q88Device::write_config(self, config).map(DeviceConfig::Block16)
    }

    fn set_gain(&mut self, db: f64) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_gain(self, db).map(DeviceConfig::Block16)
    }

    fn set_mute(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_mute(self, enabled).map(DeviceConfig::Block16)
    }

    fn set_clipguard(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_clipguard(self, enabled).map(DeviceConfig::Block16)
    }

    fn set_lowcut(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_lowcut(self, enabled).map(DeviceConfig::Block16)
    }

    fn set_output_volume(&mut self, db: f64) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_output_volume(self, db).map(DeviceConfig::Block16)
    }

    fn set_monitor_mix(&mut self, percent: f64) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_monitor_mix(self, percent).map(DeviceConfig::Block16)
    }

    fn set_dial_mode(&mut self, mode: DialMode) -> Result<DeviceConfig, DeviceError> {
        Q88Device::set_dial_mode(self, mode).map(DeviceConfig::Block16)
    }
}

pub struct Wave3Mk2Device<T> {
    transport: T,
}

impl<T: Transport> Wave3Mk2Device<T> {
    pub fn new(transport: T) -> Self {
        assert_eq!(transport.model(), DeviceModel::Wave3Mk2);
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn read_info(&mut self) -> Result<DeviceInfo, DeviceError> {
        let raw = self
            .transport
            .wave3_mk2_control_in(WAVE3_MK2_INFO_VALUE, wave3_mk2::INFO_LENGTH)?;
        DeviceInfo::from_wave3_mk2_bytes(&raw).map_err(DeviceError::from)
    }

    pub fn read_config(&mut self) -> Result<Wave3Mk2Config, DeviceError> {
        let settings = self
            .transport
            .wave3_mk2_control_in(SETTINGS_VALUE, wave3_mk2::SETTINGS_LENGTH)?;
        let headphones = self
            .transport
            .wave3_mk2_control_in(HEADPHONES_VALUE, wave3_mk2::HEADPHONES_LENGTH)?;
        let monitor = self
            .transport
            .wave3_mk2_control_in(MONITOR_VALUE, wave3_mk2::MONITOR_LENGTH)?;
        Wave3Mk2Config::from_blocks(&settings, &headphones, &monitor).map_err(DeviceError::from)
    }

    pub fn write_config(&mut self, config: &Wave3Mk2Config) -> Result<Wave3Mk2Config, DeviceError> {
        let current = self.read_config()?;
        self.write_changed(&current, config)?;
        self.readback(config)
    }

    fn update_config(
        &mut self,
        update: impl FnOnce(&Wave3Mk2Config) -> Result<Wave3Mk2Config, ProtocolError>,
    ) -> Result<Wave3Mk2Config, DeviceError> {
        let current = self.read_config()?;
        let updated = update(&current)?;
        if updated == current {
            return Ok(current);
        }
        self.write_changed(&current, &updated)?;
        self.readback(&updated)
    }

    fn write_changed(
        &mut self,
        current: &Wave3Mk2Config,
        updated: &Wave3Mk2Config,
    ) -> Result<(), DeviceError> {
        for (value, before, after) in [
            (
                SETTINGS_VALUE,
                current.settings_raw().as_slice(),
                updated.settings_raw().as_slice(),
            ),
            (
                HEADPHONES_VALUE,
                current.headphones_raw().as_slice(),
                updated.headphones_raw().as_slice(),
            ),
            (
                MONITOR_VALUE,
                current.monitor_raw().as_slice(),
                updated.monitor_raw().as_slice(),
            ),
        ] {
            if before != after {
                self.transport.wave3_mk2_control_out(value, after)?;
            }
        }
        Ok(())
    }

    fn readback(&mut self, expected: &Wave3Mk2Config) -> Result<Wave3Mk2Config, DeviceError> {
        let readback = self.read_config()?;
        if readback != *expected {
            return Err(DeviceError::ReadbackMismatch {
                expected: wave3_mk2_hex(expected),
                actual: wave3_mk2_hex(&readback),
            });
        }
        Ok(readback)
    }

    fn ensure_dsp(&mut self) -> Result<(), DeviceError> {
        if self.read_info()?.firmware_parts() < [0, 2, 3] {
            return Err(unsupported(
                "hardware EQ and dynamics on firmware older than 0.2.3",
                DeviceModel::Wave3Mk2,
            ));
        }
        Ok(())
    }
}

impl<T: Transport> WaveDriver<T> for Wave3Mk2Device<T> {
    fn spec(&self) -> &'static DeviceSpec {
        &WAVE3_MK2_SPEC
    }

    fn transport(&self) -> &T {
        &self.transport
    }

    fn read_info(&mut self) -> Result<DeviceInfo, DeviceError> {
        Wave3Mk2Device::read_info(self)
    }

    fn read_config(&mut self) -> Result<DeviceConfig, DeviceError> {
        Wave3Mk2Device::read_config(self).map(DeviceConfig::Wave3Mk2)
    }

    fn write_config(&mut self, config: &DeviceConfig) -> Result<DeviceConfig, DeviceError> {
        let DeviceConfig::Wave3Mk2(config) = config else {
            return Err(unsupported(
                "restoring another driver's configuration",
                DeviceModel::Wave3Mk2,
            ));
        };
        Wave3Mk2Device::write_config(self, config).map(DeviceConfig::Wave3Mk2)
    }

    fn set_gain(&mut self, db: f64) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| config.with_gain(db))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_mute(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_mute(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_clipguard(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_clipguard(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_lowcut(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_lowcut(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_output_volume(&mut self, db: f64) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| config.with_output_volume(db))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_monitor_mix(&mut self, percent: f64) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| config.with_monitor_mix(percent))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_clipguard_mode(&mut self, mode: ClipguardMode) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_clipguard_mode(mode)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_expander(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_expander(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_voice_tune(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_voice_tune(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_voice_tune_strength(&mut self, percent: f64) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| config.with_voice_tune_strength(percent))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_compressor(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.update_config(|config| Ok(config.with_compressor(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_eq_enabled(&mut self, enabled: bool) -> Result<DeviceConfig, DeviceError> {
        self.ensure_dsp()?;
        self.update_config(|config| Ok(config.with_eq_enabled(enabled)))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_eq_band(
        &mut self,
        index: usize,
        update: EqBandUpdate,
    ) -> Result<DeviceConfig, DeviceError> {
        self.ensure_dsp()?;
        self.update_config(|config| config.with_eq_band(index, update))
            .map(DeviceConfig::Wave3Mk2)
    }

    fn set_dynamics(&mut self, update: DynamicsUpdate) -> Result<DeviceConfig, DeviceError> {
        self.ensure_dsp()?;
        self.update_config(|config| config.with_dynamics(update))
            .map(DeviceConfig::Wave3Mk2)
    }
}

pub struct WaveDevice<T> {
    driver: Box<dyn WaveDriver<T> + Send>,
}

impl<T: Transport + 'static> WaveDevice<T> {
    pub fn new(transport: T) -> Self
    where
        T: Send,
    {
        let driver: Box<dyn WaveDriver<T> + Send> = if transport.model().is_wave3_mk2() {
            Box::new(Wave3Mk2Device::new(transport))
        } else {
            Box::new(Q88Device::new(transport))
        };
        Self { driver }
    }

    pub fn model(&self) -> DeviceModel {
        self.driver.spec().model
    }

    pub fn spec(&self) -> &'static DeviceSpec {
        self.driver.spec()
    }

    pub fn transport(&self) -> &T {
        self.driver.transport()
    }

    pub fn read_info(&mut self) -> Result<DeviceInfo, DeviceError> {
        self.driver.read_info()
    }

    pub fn read_config(&mut self) -> Result<DeviceConfig, DeviceError> {
        self.driver.read_config()
    }

    pub fn read_meter(&mut self) -> Result<Option<Vec<u8>>, DeviceError> {
        self.driver.read_meter()
    }

    pub fn write_config(&mut self, config: &DeviceConfig) -> Result<DeviceConfig, DeviceError> {
        self.driver.write_config(config)
    }

    pub fn apply(&mut self, setting: Setting) -> Result<DeviceConfig, DeviceError> {
        self.driver.apply(setting)
    }
}

fn unsupported(feature: &'static str, model: DeviceModel) -> DeviceError {
    ProtocolError::Unsupported {
        feature,
        model: model.display_name(),
    }
    .into()
}

fn wave3_mk2_hex(config: &Wave3Mk2Config) -> String {
    [
        format_hex(config.settings_raw()),
        format_hex(config.headphones_raw()),
        format_hex(config.monitor_raw()),
    ]
    .join(" | ")
}
