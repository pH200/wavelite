use serde::Serialize;

use crate::protocol::{ProtocolError, format_hex};

pub const SETTINGS_LENGTH: usize = 38;
pub const HEADPHONES_LENGTH: usize = 2;
pub const MONITOR_LENGTH: usize = 6;
pub const INFO_LENGTH: usize = 117;
pub const SETTINGS_VALUE: u16 = 0x0004;
pub const HEADPHONES_VALUE: u16 = 0x0005;
pub const MONITOR_VALUE: u16 = 0x0001;
pub const INFO_VALUE: u16 = 0x0000;

const MUTE_MASK: u8 = 0x01;
const LOWCUT_MASK: u8 = 0x02;
const EXPANDER_MASK: u8 = 0x20;
const VOICE_TUNE_MASK: u8 = 0x40;
const COMPRESSOR_MASK: u8 = 0x80;
const LIMITER_MODE_MASK: u8 = 0x1c;
const LIMITER_MODE_SHIFT: u8 = 2;
const EQ_MASK: u8 = 0x01;
const EQ_FIRST_BAND: usize = 18;
const EQ_BAND_STRIDE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipguardMode {
    Enabled,
    Bypassed,
    HardClip,
}

impl ClipguardMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Bypassed => "bypassed",
            Self::HardClip => "hard_clip",
        }
    }

    fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Enabled),
            1 => Some(Self::Bypassed),
            2 => Some(Self::HardClip),
            _ => None,
        }
    }

    fn raw(self) -> u8 {
        match self {
            Self::Enabled => 0,
            Self::Bypassed => 1,
            Self::HardClip => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqBandType {
    HighPass,
    LowShelf,
    Bell,
    Notch,
    HighShelf,
    LowPass,
}

impl EqBandType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HighPass => "high_pass",
            Self::LowShelf => "low_shelf",
            Self::Bell => "bell",
            Self::Notch => "notch",
            Self::HighShelf => "high_shelf",
            Self::LowPass => "low_pass",
        }
    }

    fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::HighPass),
            1 => Some(Self::LowShelf),
            2 => Some(Self::Bell),
            3 => Some(Self::Notch),
            4 => Some(Self::HighShelf),
            5 => Some(Self::LowPass),
            _ => None,
        }
    }

    fn raw(self) -> u8 {
        match self {
            Self::HighPass => 0,
            Self::LowShelf => 1,
            Self::Bell => 2,
            Self::Notch => 3,
            Self::HighShelf => 4,
            Self::LowPass => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EqBandUpdate {
    pub enabled: Option<bool>,
    pub kind: Option<EqBandType>,
    pub gain_db: Option<f64>,
    pub frequency_hz: Option<f64>,
    pub q: Option<f64>,
}

impl EqBandUpdate {
    pub fn is_empty(self) -> bool {
        self.enabled.is_none()
            && self.kind.is_none()
            && self.gain_db.is_none()
            && self.frequency_hz.is_none()
            && self.q.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DynamicsUpdate {
    pub compressor_threshold_db: Option<f64>,
    pub compressor_ratio: Option<f64>,
    pub compressor_attack_ms: Option<f64>,
    pub compressor_release_ms: Option<f64>,
    pub compressor_makeup_gain_db: Option<f64>,
    pub expander_threshold_db: Option<f64>,
    pub expander_ratio: Option<f64>,
    pub expander_attack_ms: Option<f64>,
    pub expander_release_ms: Option<f64>,
    pub expander_max_reduction_db: Option<f64>,
}

impl DynamicsUpdate {
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave3Mk2Config {
    settings: [u8; SETTINGS_LENGTH],
    headphones: [u8; HEADPHONES_LENGTH],
    monitor: [u8; MONITOR_LENGTH],
}

#[derive(Debug, Serialize, PartialEq)]
pub struct EqBandView {
    pub enabled: bool,
    pub kind: String,
    pub gain_db: f64,
    pub frequency_hz: u16,
    pub q: f64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct EqView {
    pub enabled: bool,
    pub bands: [EqBandView; 4],
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CompressorView {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: u16,
    pub release_ms: u16,
    pub makeup_gain_db: u8,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ExpanderView {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: u16,
    pub release_ms: u16,
    pub max_reduction_db: u8,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Wave3Mk2ConfigView {
    pub settings_hex: String,
    pub headphones_hex: String,
    pub monitor_hex: String,
    pub gain_db: f64,
    pub muted: bool,
    pub clipguard: bool,
    pub clipguard_mode: String,
    pub lowcut: bool,
    pub expander: bool,
    pub voice_tune: bool,
    pub voice_tune_strength_percent: u8,
    pub compressor: bool,
    pub output_volume_db: f64,
    pub monitor_mix_percent: f64,
    pub eq: EqView,
    pub compressor_settings: CompressorView,
    pub expander_settings: ExpanderView,
}

impl Wave3Mk2Config {
    pub fn from_blocks(
        settings: &[u8],
        headphones: &[u8],
        monitor: &[u8],
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            settings: exact_block("Wave:3 MK.2 settings", SETTINGS_LENGTH, settings)?,
            headphones: exact_block("Wave:3 MK.2 headphones", HEADPHONES_LENGTH, headphones)?,
            monitor: exact_block("Wave:3 MK.2 monitor", MONITOR_LENGTH, monitor)?,
        })
    }

    pub fn settings_raw(&self) -> &[u8; SETTINGS_LENGTH] {
        &self.settings
    }

    pub fn headphones_raw(&self) -> &[u8; HEADPHONES_LENGTH] {
        &self.headphones
    }

    pub fn monitor_raw(&self) -> &[u8; MONITOR_LENGTH] {
        &self.monitor
    }

    pub fn gain_db(&self) -> f64 {
        f64::from(self.settings[0])
    }

    pub fn muted(&self) -> bool {
        self.settings[1] & MUTE_MASK != 0
    }

    pub fn lowcut(&self) -> bool {
        self.settings[1] & LOWCUT_MASK != 0
    }

    pub fn expander(&self) -> bool {
        self.settings[1] & EXPANDER_MASK != 0
    }

    pub fn voice_tune(&self) -> bool {
        self.settings[1] & VOICE_TUNE_MASK != 0
    }

    pub fn compressor(&self) -> bool {
        self.settings[1] & COMPRESSOR_MASK != 0
    }

    pub fn voice_tune_strength_percent(&self) -> u8 {
        self.settings[10]
    }

    pub fn clipguard_mode_raw(&self) -> u8 {
        (self.settings[2] & LIMITER_MODE_MASK) >> LIMITER_MODE_SHIFT
    }

    pub fn clipguard_mode(&self) -> Option<ClipguardMode> {
        ClipguardMode::from_raw(self.clipguard_mode_raw())
    }

    pub fn clipguard(&self) -> bool {
        self.clipguard_mode() == Some(ClipguardMode::Enabled)
    }

    pub fn output_volume_db(&self) -> f64 {
        -f64::from(self.headphones[0]) / 4.0
    }

    pub fn monitor_mix_percent(&self) -> f64 {
        f64::from(self.monitor[0]) / 2.0
    }

    pub fn with_gain(&self, db: f64) -> Result<Self, ProtocolError> {
        finite(db)?;
        if !(0.0..=40.0).contains(&db) {
            return Err(ProtocolError::GainRange);
        }
        let mut updated = self.clone();
        updated.settings[0] = db.round_ties_even() as u8;
        Ok(updated)
    }

    pub fn with_mute(&self, enabled: bool) -> Self {
        self.with_flag(MUTE_MASK, enabled)
    }

    pub fn with_lowcut(&self, enabled: bool) -> Self {
        self.with_flag(LOWCUT_MASK, enabled)
    }

    pub fn with_expander(&self, enabled: bool) -> Self {
        self.with_flag(EXPANDER_MASK, enabled)
    }

    pub fn with_voice_tune(&self, enabled: bool) -> Self {
        self.with_flag(VOICE_TUNE_MASK, enabled)
    }

    pub fn with_compressor(&self, enabled: bool) -> Self {
        self.with_flag(COMPRESSOR_MASK, enabled)
    }

    pub fn with_voice_tune_strength(&self, percent: f64) -> Result<Self, ProtocolError> {
        finite(percent)?;
        if !(0.0..=100.0).contains(&percent) {
            return Err(ProtocolError::InvalidValue(
                "voice tune strength must be between 0 and 100 percent",
            ));
        }
        let mut updated = self.clone();
        updated.settings[10] = percent.round_ties_even() as u8;
        Ok(updated)
    }

    pub fn with_clipguard(&self, enabled: bool) -> Self {
        self.with_clipguard_mode(if enabled {
            ClipguardMode::Enabled
        } else {
            ClipguardMode::HardClip
        })
    }

    pub fn with_clipguard_mode(&self, mode: ClipguardMode) -> Self {
        let mut updated = self.clone();
        updated.settings[2] =
            (updated.settings[2] & !LIMITER_MODE_MASK) | (mode.raw() << LIMITER_MODE_SHIFT);
        updated
    }

    pub fn with_output_volume(&self, db: f64) -> Result<Self, ProtocolError> {
        finite(db)?;
        if !(-60.0..=0.0).contains(&db) {
            return Err(ProtocolError::InvalidValue(
                "Wave:3 MK.2 output volume must be between -60 and 0 dB",
            ));
        }
        let mut updated = self.clone();
        updated.headphones[0] = (-db * 4.0).round_ties_even() as u8;
        Ok(updated)
    }

    pub fn with_monitor_mix(&self, percent: f64) -> Result<Self, ProtocolError> {
        finite(percent)?;
        if !(0.0..=100.0).contains(&percent) {
            return Err(ProtocolError::MonitorMixRange);
        }
        let mut updated = self.clone();
        updated.monitor[0] = (percent * 2.0).round_ties_even() as u8;
        Ok(updated)
    }

    pub fn with_eq_enabled(&self, enabled: bool) -> Self {
        let mut updated = self.clone();
        updated.settings[2] = (updated.settings[2] & !EQ_MASK) | u8::from(enabled);
        updated
    }

    fn with_flag(&self, mask: u8, enabled: bool) -> Self {
        let mut updated = self.clone();
        updated.settings[1] = (updated.settings[1] & !mask) | if enabled { mask } else { 0 };
        updated
    }
}

fn exact_block<const N: usize>(
    block: &'static str,
    expected: usize,
    raw: &[u8],
) -> Result<[u8; N], ProtocolError> {
    raw.try_into()
        .map_err(|_| ProtocolError::InvalidBlockLength {
            block,
            expected,
            actual: raw.len(),
        })
}

fn finite(value: f64) -> Result<(), ProtocolError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProtocolError::NonFinite)
    }
}

impl Wave3Mk2Config {
    pub fn eq_enabled(&self) -> bool {
        self.settings[2] & EQ_MASK != 0
    }

    pub fn with_eq_band(&self, index: usize, update: EqBandUpdate) -> Result<Self, ProtocolError> {
        if !(1..=4).contains(&index) {
            return Err(ProtocolError::EqBandIndex);
        }
        if let Some(gain) = update.gain_db {
            finite(gain)?;
            if !(-12.0..=12.0).contains(&gain) {
                return Err(ProtocolError::EqSetting(
                    "band gain must be between -12 and 12 dB",
                ));
            }
        }
        if let Some(frequency) = update.frequency_hz {
            finite(frequency)?;
            if !(20.0..=20_000.0).contains(&frequency) {
                return Err(ProtocolError::EqSetting(
                    "band frequency must be between 20 and 20000 Hz",
                ));
            }
        }
        if let Some(q) = update.q {
            finite(q)?;
            if !(0.1..=10.0).contains(&q) {
                return Err(ProtocolError::EqSetting(
                    "band Q must be between 0.1 and 10",
                ));
            }
        }

        let mut updated = self.clone();
        let offset = EQ_FIRST_BAND + (index - 1) * EQ_BAND_STRIDE;
        if let Some(enabled) = update.enabled {
            updated.settings[offset] = (updated.settings[offset] & !1) | u8::from(enabled);
        }
        if let Some(kind) = update.kind {
            updated.settings[offset] = (updated.settings[offset] & !0x0e) | (kind.raw() << 1);
        }
        if let Some(gain) = update.gain_db {
            updated.settings[offset + 1] = (gain * 4.0).round_ties_even() as i8 as u8;
        }
        if let Some(frequency) = update.frequency_hz {
            let raw = frequency.round_ties_even() as u16;
            updated.settings[offset + 2..offset + 4].copy_from_slice(&raw.to_le_bytes());
        }
        if let Some(q) = update.q {
            updated.settings[offset + 4] = (q.log10() * 64.0).round_ties_even() as i8 as u8;
        }
        Ok(updated)
    }

    pub fn with_dynamics(&self, update: DynamicsUpdate) -> Result<Self, ProtocolError> {
        validate_dynamics(update)?;
        let mut updated = self.clone();
        if let Some(value) = update.expander_threshold_db {
            updated.settings[3] = value.round_ties_even() as i8 as u8;
        }
        if let Some(value) = update.expander_ratio {
            updated.settings[4] = (value * 16.0).round_ties_even() as u8;
        }
        if let Some(value) = update.expander_attack_ms {
            put_u16(&mut updated.settings, 5, value.round_ties_even() as u16);
        }
        if let Some(value) = update.expander_release_ms {
            put_u16(&mut updated.settings, 7, value.round_ties_even() as u16);
        }
        if let Some(value) = update.expander_max_reduction_db {
            updated.settings[9] = value.round_ties_even() as u8;
        }
        if let Some(value) = update.compressor_threshold_db {
            updated.settings[11] = value.round_ties_even() as i8 as u8;
        }
        if let Some(value) = update.compressor_ratio {
            updated.settings[12] = value.round_ties_even() as u8;
        }
        if let Some(value) = update.compressor_attack_ms {
            put_u16(&mut updated.settings, 13, value.round_ties_even() as u16);
        }
        if let Some(value) = update.compressor_release_ms {
            put_u16(&mut updated.settings, 15, value.round_ties_even() as u16);
        }
        if let Some(value) = update.compressor_makeup_gain_db {
            updated.settings[17] = value.round_ties_even() as u8;
        }
        Ok(updated)
    }

    pub fn view(&self) -> Wave3Mk2ConfigView {
        Wave3Mk2ConfigView {
            settings_hex: format_hex(&self.settings),
            headphones_hex: format_hex(&self.headphones),
            monitor_hex: format_hex(&self.monitor),
            gain_db: self.gain_db(),
            muted: self.muted(),
            clipguard: self.clipguard(),
            clipguard_mode: self.clipguard_mode().map_or_else(
                || format!("unknown_{}", self.clipguard_mode_raw()),
                |mode| mode.as_str().to_owned(),
            ),
            lowcut: self.lowcut(),
            expander: self.expander(),
            voice_tune: self.voice_tune(),
            voice_tune_strength_percent: self.voice_tune_strength_percent(),
            compressor: self.compressor(),
            output_volume_db: self.output_volume_db(),
            monitor_mix_percent: self.monitor_mix_percent(),
            eq: EqView {
                enabled: self.eq_enabled(),
                bands: std::array::from_fn(|index| self.eq_band_view(index)),
            },
            compressor_settings: CompressorView {
                threshold_db: f64::from(self.settings[11] as i8),
                ratio: f64::from(self.settings[12]),
                attack_ms: read_u16(&self.settings, 13),
                release_ms: read_u16(&self.settings, 15),
                makeup_gain_db: self.settings[17],
            },
            expander_settings: ExpanderView {
                threshold_db: f64::from(self.settings[3] as i8),
                ratio: f64::from(self.settings[4]) / 16.0,
                attack_ms: read_u16(&self.settings, 5),
                release_ms: read_u16(&self.settings, 7),
                max_reduction_db: self.settings[9],
            },
        }
    }

    fn eq_band_view(&self, index: usize) -> EqBandView {
        let offset = EQ_FIRST_BAND + index * EQ_BAND_STRIDE;
        let raw_kind = (self.settings[offset] & 0x0e) >> 1;
        EqBandView {
            enabled: self.settings[offset] & 1 != 0,
            kind: EqBandType::from_raw(raw_kind).map_or_else(
                || format!("unknown_{raw_kind}"),
                |kind| kind.as_str().to_owned(),
            ),
            gain_db: f64::from(self.settings[offset + 1] as i8) / 4.0,
            frequency_hz: read_u16(&self.settings, offset + 2),
            q: 10.0_f64.powf(f64::from(self.settings[offset + 4] as i8) / 64.0),
        }
    }
}

fn validate_dynamics(update: DynamicsUpdate) -> Result<(), ProtocolError> {
    for (value, minimum, maximum, message) in [
        (
            update.compressor_threshold_db,
            -60.0,
            -1.0,
            "compressor threshold must be between -60 and -1 dB",
        ),
        (
            update.compressor_ratio,
            1.0,
            20.0,
            "compressor ratio must be between 1 and 20",
        ),
        (
            update.compressor_attack_ms,
            1.0,
            300.0,
            "compressor attack must be between 1 and 300 ms",
        ),
        (
            update.compressor_release_ms,
            50.0,
            1000.0,
            "compressor release must be between 50 and 1000 ms",
        ),
        (
            update.compressor_makeup_gain_db,
            0.0,
            20.0,
            "compressor makeup gain must be between 0 and 20 dB",
        ),
        (
            update.expander_threshold_db,
            -60.0,
            -1.0,
            "expander threshold must be between -60 and -1 dB",
        ),
        (
            update.expander_ratio,
            1.0,
            3.0,
            "expander ratio must be between 1 and 3",
        ),
        (
            update.expander_attack_ms,
            1.0,
            200.0,
            "expander attack must be between 1 and 200 ms",
        ),
        (
            update.expander_release_ms,
            10.0,
            1000.0,
            "expander release must be between 10 and 1000 ms",
        ),
        (
            update.expander_max_reduction_db,
            0.0,
            60.0,
            "expander maximum reduction must be between 0 and 60 dB",
        ),
    ] {
        if let Some(value) = value {
            finite(value)?;
            if !(minimum..=maximum).contains(&value) {
                return Err(ProtocolError::DynamicsSetting(message));
            }
        }
    }
    Ok(())
}

fn read_u16(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn put_u16(raw: &mut [u8], offset: usize, value: u16) {
    raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
