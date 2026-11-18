use serde::Serialize;
use thiserror::Error;

pub const CONFIG_LENGTH: usize = 16;
pub const METER_LENGTH: usize = 8;
pub const INFO_LENGTH: usize = 64;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("config must be exactly {CONFIG_LENGTH} bytes")]
    InvalidConfigLength,
    #[error("device info must be exactly {INFO_LENGTH} bytes")]
    InvalidInfoLength,
    #[error("invalid config hex")]
    InvalidConfigHex,
    #[error("device serial must be ASCII")]
    InvalidSerial,
    #[error("value must be finite")]
    NonFinite,
    #[error("input gain must be between 0 and 40 dB")]
    GainRange,
    #[error("output volume must be between -128 and 0 dB")]
    OutputVolumeRange,
    #[error("monitor mix must be between 0 and 100 percent")]
    MonitorMixRange,
    #[error("{block} must be exactly {expected} bytes, got {actual}")]
    InvalidBlockLength {
        block: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{feature} is not supported by {model}")]
    Unsupported {
        feature: &'static str,
        model: &'static str,
    },
    #[error("EQ band must be between 1 and 4")]
    EqBandIndex,
    #[error("invalid EQ setting: {0}")]
    EqSetting(&'static str),
    #[error("invalid dynamics setting: {0}")]
    DynamicsSetting(&'static str),
    #[error("unknown Clipguard mode: {0}")]
    ClipguardMode(u8),
    #[error("{0}")]
    InvalidValue(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DialMode {
    InputGain = 1,
    OutputVolume = 2,
    MonitorMix = 3,
}

impl DialMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::InputGain => "input_gain",
            Self::OutputVolume => "output_volume",
            Self::MonitorMix => "monitor_mix",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    raw: [u8; CONFIG_LENGTH],
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ConfigView {
    pub raw_hex: String,
    pub gain_db: f64,
    pub muted: bool,
    pub clipguard: bool,
    pub lowcut: bool,
    pub output_volume_db: f64,
    pub monitor_mix_percent: f64,
    pub dial_mode: String,
}

impl Config {
    pub fn from_bytes(raw: &[u8]) -> Result<Self, ProtocolError> {
        let raw: [u8; CONFIG_LENGTH] = raw
            .try_into()
            .map_err(|_| ProtocolError::InvalidConfigLength)?;
        Ok(Self { raw })
    }

    pub fn from_hex(value: &str) -> Result<Self, ProtocolError> {
        let digits: Vec<u8> = value
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        if digits.len() != CONFIG_LENGTH * 2 {
            return Err(ProtocolError::InvalidConfigLength);
        }
        let mut raw = [0; CONFIG_LENGTH];
        for (index, byte) in raw.iter_mut().enumerate() {
            let high = hex_digit(digits[index * 2])?;
            let low = hex_digit(digits[index * 2 + 1])?;
            *byte = (high << 4) | low;
        }
        Ok(Self { raw })
    }

    pub fn raw(&self) -> &[u8; CONFIG_LENGTH] {
        &self.raw
    }

    pub fn gain_raw(&self) -> u16 {
        u16::from_le_bytes([self.raw[0], self.raw[1]])
    }

    pub fn gain_db(&self) -> f64 {
        f64::from(self.gain_raw()) / 256.0
    }

    pub fn muted(&self) -> bool {
        self.raw[4] != 0
    }

    pub fn clipguard(&self) -> bool {
        self.raw[5] != 0
    }

    pub fn lowcut(&self) -> bool {
        self.raw[6] != 0
    }

    pub fn output_volume_db(&self) -> f64 {
        f64::from(i16::from_le_bytes([self.raw[7], self.raw[8]])) / 256.0
    }

    pub fn monitor_mix_percent(&self) -> f64 {
        f64::from(u16::from_le_bytes([self.raw[10], self.raw[11]])) / 256.0
    }

    pub fn dial_mode(&self) -> Option<DialMode> {
        match self.raw[12] {
            1 => Some(DialMode::InputGain),
            2 => Some(DialMode::OutputVolume),
            3 => Some(DialMode::MonitorMix),
            _ => None,
        }
    }

    pub fn with_gain(&self, db: f64) -> Result<Self, ProtocolError> {
        finite(db)?;
        if !(0.0..=40.0).contains(&db) {
            return Err(ProtocolError::GainRange);
        }
        let mut updated = self.clone();
        updated.raw[..2].copy_from_slice(&((db * 256.0).round_ties_even() as u16).to_le_bytes());
        Ok(updated)
    }

    pub fn with_mute(&self, enabled: bool) -> Self {
        self.replace(4, u8::from(enabled))
    }

    pub fn with_clipguard(&self, enabled: bool) -> Self {
        self.replace(5, u8::from(enabled))
    }

    pub fn with_lowcut(&self, enabled: bool) -> Self {
        self.replace(6, u8::from(enabled))
    }

    pub fn with_output_volume(&self, db: f64) -> Result<Self, ProtocolError> {
        finite(db)?;
        if !(-128.0..=0.0).contains(&db) {
            return Err(ProtocolError::OutputVolumeRange);
        }
        let mut updated = self.clone();
        updated.raw[7..9].copy_from_slice(&((db * 256.0) as i16).to_le_bytes());
        Ok(updated)
    }

    pub fn with_monitor_mix(&self, percent: f64) -> Result<Self, ProtocolError> {
        finite(percent)?;
        if !(0.0..=100.0).contains(&percent) {
            return Err(ProtocolError::MonitorMixRange);
        }
        let mut updated = self.clone();
        updated.raw[10..12]
            .copy_from_slice(&((percent * 256.0).round_ties_even() as u16).to_le_bytes());
        Ok(updated)
    }

    pub fn with_dial_mode(&self, mode: DialMode) -> Self {
        self.replace(12, mode as u8)
    }

    pub fn view(&self) -> ConfigView {
        ConfigView {
            raw_hex: format_hex(&self.raw),
            gain_db: self.gain_db(),
            muted: self.muted(),
            clipguard: self.clipguard(),
            lowcut: self.lowcut(),
            output_volume_db: self.output_volume_db(),
            monitor_mix_percent: self.monitor_mix_percent(),
            dial_mode: self.dial_mode().map_or_else(
                || format!("unknown_{}", self.raw[12]),
                |mode| mode.as_str().to_owned(),
            ),
        }
    }

    fn replace(&self, offset: usize, value: u8) -> Self {
        let mut updated = self.clone();
        updated.raw[offset] = value;
        updated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    raw: Vec<u8>,
    wave3_mk2: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DeviceInfoView {
    pub api_version: String,
    pub firmware_version: String,
    pub serial: String,
    pub raw_hex: String,
}

impl DeviceInfo {
    pub fn from_bytes(raw: &[u8]) -> Result<Self, ProtocolError> {
        if raw.len() != INFO_LENGTH {
            return Err(ProtocolError::InvalidInfoLength);
        }
        Self::new(raw.to_vec(), false)
    }

    pub fn from_wave3_mk2_bytes(raw: &[u8]) -> Result<Self, ProtocolError> {
        if raw.len() != crate::wave3_mk2::INFO_LENGTH {
            return Err(ProtocolError::InvalidBlockLength {
                block: "Wave:3 MK.2 device info",
                expected: crate::wave3_mk2::INFO_LENGTH,
                actual: raw.len(),
            });
        }
        Self::new(raw.to_vec(), true)
    }

    fn new(raw: Vec<u8>, wave3_mk2: bool) -> Result<Self, ProtocolError> {
        let info = Self { raw, wave3_mk2 };
        if !info.serial_bytes().is_ascii() {
            return Err(ProtocolError::InvalidSerial);
        }
        Ok(info)
    }

    pub fn api_version(&self) -> String {
        let offset = usize::from(self.wave3_mk2);
        format!("{}.{}", self.raw[offset], self.raw[offset + 1])
    }

    pub fn firmware_version(&self) -> String {
        self.firmware_parts()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }

    pub fn firmware_parts(&self) -> [u8; 3] {
        let offset = if self.wave3_mk2 { 5 } else { 21 };
        self.raw[offset..offset + 3]
            .try_into()
            .expect("device info length validated")
    }

    pub fn serial(&self) -> String {
        String::from_utf8(self.serial_bytes().to_vec()).expect("serial validated as ASCII")
    }

    pub fn view(&self) -> DeviceInfoView {
        DeviceInfoView {
            api_version: self.api_version(),
            firmware_version: self.firmware_version(),
            serial: self.serial(),
            raw_hex: format_hex(&self.raw),
        }
    }

    fn serial_bytes(&self) -> &[u8] {
        let serial = if self.wave3_mk2 {
            &self.raw[35..53]
        } else {
            &self.raw[36..48]
        };
        let end = serial
            .iter()
            .rposition(|byte| *byte != 0)
            .map_or(0, |index| index + 1);
        &serial[..end]
    }
}

fn finite(value: f64) -> Result<(), ProtocolError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProtocolError::NonFinite)
    }
}

fn hex_digit(byte: u8) -> Result<u8, ProtocolError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ProtocolError::InvalidConfigHex),
    }
}

pub fn decode_hex(
    value: &str,
    expected: usize,
    block: &'static str,
) -> Result<Vec<u8>, ProtocolError> {
    let digits: Vec<u8> = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if digits.len() != expected * 2 {
        return Err(ProtocolError::InvalidBlockLength {
            block,
            expected,
            actual: digits.len() / 2,
        });
    }
    digits
        .chunks_exact(2)
        .map(|pair| Ok((hex_digit(pair[0])? << 4) | hex_digit(pair[1])?))
        .collect()
}

pub fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
