use wavelite::{
    device::{
        CONFIG_VALUE, DeviceConfig, DeviceError, DeviceModel, Setting, Transport, WAVE1_SPEC,
        WAVE3_MK2_SPEC, WAVE3_SPEC, Wave1Device, WaveDevice,
    },
    protocol::{Config, DeviceInfo, DialMode, ProtocolError},
    wave3_mk2::{
        self, ClipguardMode, DynamicsUpdate, EqBandType, EqBandUpdate, HEADPHONES_VALUE,
        INFO_VALUE as WAVE3_MK2_INFO_VALUE, MONITOR_VALUE, SETTINGS_VALUE, Wave3Mk2Config,
    },
};

const SETTINGS: [u8; 38] = [
    0x22, 0x62, 0x00, 0xd4, 0x20, 0x0a, 0x00, 0xc8, 0x00, 0x14, 0x10, 0xf4, 0x04, 0x0a, 0x00, 0x64,
    0x00, 0x06, 0x01, 0x00, 0x3c, 0x00, 0xf6, 0x03, 0x0d, 0xe3, 0x00, 0x00, 0x05, 0xf4, 0x5c, 0x03,
    0xf2, 0x25, 0x07, 0xfd, 0x27, 0xe8,
];
const HEADPHONES: [u8; 2] = [0x30, 0x00];
const MONITOR: [u8; 6] = [0x32, 0x01, 0x02, 0x03, 0x02, 0x00];

fn baseline() -> Wave3Mk2Config {
    Wave3Mk2Config::from_blocks(&SETTINGS, &HEADPHONES, &MONITOR).unwrap()
}

fn info(version: [u8; 3]) -> [u8; 117] {
    let mut info = [0; 117];
    info[..5].copy_from_slice(&[1, 5, 2, 2, 0]);
    info[5..8].copy_from_slice(&version);
    info[35..47].copy_from_slice(b"M2Q8R4T6V9X1");
    info
}

struct FakeWave3Mk2Transport {
    config: Wave3Mk2Config,
    info: [u8; 117],
    writes: Vec<(u16, Vec<u8>)>,
    corrupt_readback: bool,
    short_settings: bool,
}

impl FakeWave3Mk2Transport {
    fn new() -> Self {
        Self {
            config: baseline(),
            info: info([0, 2, 3]),
            writes: Vec::new(),
            corrupt_readback: false,
            short_settings: false,
        }
    }
}

impl Transport for FakeWave3Mk2Transport {
    fn control_in(&mut self, _value: u16, _length: usize) -> Result<Vec<u8>, DeviceError> {
        panic!("block16 read on Wave:3 MK.2")
    }

    fn control_out(&mut self, _value: u16, _raw: &[u8]) -> Result<(), DeviceError> {
        panic!("block16 write on Wave:3 MK.2")
    }

    fn wave3_mk2_control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
        match value {
            SETTINGS_VALUE => {
                if self.short_settings {
                    return Ok(self.config.settings_raw()[..37].to_vec());
                }
                let mut raw = *self.config.settings_raw();
                if self.corrupt_readback && !self.writes.is_empty() {
                    self.corrupt_readback = false;
                    raw[0] ^= 1;
                }
                assert_eq!(length, wave3_mk2::SETTINGS_LENGTH);
                Ok(raw.to_vec())
            }
            HEADPHONES_VALUE => {
                assert_eq!(length, wave3_mk2::HEADPHONES_LENGTH);
                Ok(self.config.headphones_raw().to_vec())
            }
            MONITOR_VALUE => {
                assert_eq!(length, wave3_mk2::MONITOR_LENGTH);
                Ok(self.config.monitor_raw().to_vec())
            }
            WAVE3_MK2_INFO_VALUE => {
                assert_eq!(length, wave3_mk2::INFO_LENGTH);
                Ok(self.info.to_vec())
            }
            _ => panic!("unexpected MK.2 read selector {value}"),
        }
    }

    fn wave3_mk2_control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        self.writes.push((value, raw.to_vec()));
        let current = self.config.clone();
        match value {
            SETTINGS_VALUE => {
                self.config = Wave3Mk2Config::from_blocks(
                    raw,
                    current.headphones_raw(),
                    current.monitor_raw(),
                )
                .unwrap();
            }
            HEADPHONES_VALUE => {
                self.config =
                    Wave3Mk2Config::from_blocks(current.settings_raw(), raw, current.monitor_raw())
                        .unwrap();
            }
            MONITOR_VALUE => {
                self.config = Wave3Mk2Config::from_blocks(
                    current.settings_raw(),
                    current.headphones_raw(),
                    raw,
                )
                .unwrap();
            }
            _ => panic!("unexpected Wave:3 MK.2 write selector {value}"),
        }
        Ok(())
    }

    fn model(&self) -> DeviceModel {
        DeviceModel::Wave3Mk2
    }
}

#[test]
fn decodes_wave3_mk2_blocks_and_identity() {
    let config = baseline();
    assert_eq!(config.gain_db(), 34.0);
    assert!(!config.muted());
    assert!(config.lowcut());
    assert!(config.expander());
    assert!(config.voice_tune());
    assert_eq!(config.voice_tune_strength_percent(), 16);
    assert!(!config.compressor());
    assert_eq!(config.clipguard_mode(), Some(ClipguardMode::Enabled));
    assert_eq!(config.output_volume_db(), -12.0);
    assert_eq!(config.monitor_mix_percent(), 25.0);
    assert!(!config.eq_enabled());

    let view = config.view();
    assert_eq!(view.eq.bands[0].kind, "high_pass");
    assert_eq!(view.eq.bands[0].gain_db, 0.0);
    assert_eq!(view.eq.bands[0].frequency_hz, 60);
    assert_eq!(view.compressor_settings.threshold_db, -12.0);
    assert_eq!(view.expander_settings.ratio, 2.0);

    let info = DeviceInfo::from_wave3_mk2_bytes(&info([0, 2, 3])).unwrap();
    assert_eq!(info.api_version(), "5.2");
    assert_eq!(info.firmware_version(), "0.2.3");
    assert_eq!(info.serial(), "M2Q8R4T6V9X1");
}

#[test]
fn rejects_wrong_wave3_mk2_lengths() {
    assert_eq!(
        Wave3Mk2Config::from_blocks(&[0; 37], &HEADPHONES, &MONITOR).unwrap_err(),
        ProtocolError::InvalidBlockLength {
            block: "Wave:3 MK.2 settings",
            expected: 38,
            actual: 37,
        }
    );
    assert!(DeviceInfo::from_wave3_mk2_bytes(&[0; 116]).is_err());
}

#[test]
fn core_controls_write_only_changed_blocks() {
    let mut device = WaveDevice::new(FakeWave3Mk2Transport::new());
    let config = device.apply(Setting::Gain(30.0)).unwrap();
    assert_eq!(config.gain_db(), 30.0);
    assert_eq!(device.transport().writes.len(), 1);
    assert_eq!(device.transport().writes[0].0, SETTINGS_VALUE);
    assert_eq!(device.transport().writes[0].1[0], 30);
    assert_eq!(device.transport().writes[0].1[1..], SETTINGS[1..]);

    device.apply(Setting::Mute(true)).unwrap();
    device.apply(Setting::Lowcut(false)).unwrap();
    device.apply(Setting::Expander(false)).unwrap();
    device.apply(Setting::VoiceTune(false)).unwrap();
    device.apply(Setting::Compressor(true)).unwrap();
    device.apply(Setting::VoiceTuneStrength(100.0)).unwrap();
    let settings = &device.transport().config;
    assert_eq!(settings.settings_raw()[1], 0x81);
    assert_eq!(settings.voice_tune_strength_percent(), 100);

    device.apply(Setting::OutputVolume(-20.0)).unwrap();
    device.apply(Setting::MonitorMix(75.0)).unwrap();
    let transport = device.transport();
    assert_eq!(transport.config.output_volume_db(), -20.0);
    assert_eq!(transport.config.monitor_mix_percent(), 75.0);
    assert_eq!(transport.config.headphones_raw(), &[80, 0]);
    assert_eq!(transport.config.monitor_raw()[0], 150);
    assert!(
        transport
            .writes
            .iter()
            .any(|(value, raw)| { *value == HEADPHONES_VALUE && raw.as_slice() == [80, 0] })
    );
    assert!(
        transport.writes.iter().any(|(value, raw)| {
            *value == MONITOR_VALUE && raw.as_slice() == [150, 1, 2, 3, 2, 0]
        })
    );
}

#[test]
fn clipguard_modes_preserve_unrelated_bits() {
    let mut raw = SETTINGS;
    raw[2] = 0xa1;
    let config = Wave3Mk2Config::from_blocks(&raw, &HEADPHONES, &MONITOR).unwrap();
    for (mode, field) in [
        (ClipguardMode::Enabled, 0),
        (ClipguardMode::Bypassed, 0x04),
        (ClipguardMode::HardClip, 0x08),
    ] {
        let updated = config.with_clipguard_mode(mode);
        assert_eq!(updated.settings_raw()[2], 0xa1 | field);
        assert_eq!(updated.clipguard_mode(), Some(mode));
        assert_eq!(updated.settings_raw()[..2], raw[..2]);
        assert_eq!(updated.settings_raw()[3..], raw[3..]);
    }
    assert!(!config.with_clipguard(false).clipguard());
    assert_eq!(
        config.with_clipguard(false).clipguard_mode(),
        Some(ClipguardMode::HardClip)
    );
}

#[test]
fn eq_updates_only_the_selected_fields() {
    let mut device = WaveDevice::new(FakeWave3Mk2Transport::new());
    device.apply(Setting::EqEnabled(true)).unwrap();
    device
        .apply(Setting::EqBand {
            index: 1,
            update: EqBandUpdate {
                enabled: Some(true),
                kind: Some(EqBandType::Bell),
                gain_db: Some(3.0),
                frequency_hz: Some(1000.0),
                q: Some(1.0),
            },
        })
        .unwrap();
    let raw = device.transport().config.settings_raw();
    assert_eq!(raw[2], 1);
    assert_eq!(raw[18], 5);
    assert_eq!(raw[19], 12);
    assert_eq!(&raw[20..22], &[0xe8, 0x03]);
    assert_eq!(raw[22], 0);
    assert_eq!(&raw[3..18], &SETTINGS[3..18]);
    assert_eq!(&raw[23..], &SETTINGS[23..]);
    assert!(
        device
            .transport()
            .writes
            .iter()
            .all(|(value, _)| { *value == SETTINGS_VALUE })
    );

    for setting in [
        Setting::EqBand {
            index: 0,
            update: EqBandUpdate::default(),
        },
        Setting::EqBand {
            index: 1,
            update: EqBandUpdate {
                gain_db: Some(12.1),
                ..EqBandUpdate::default()
            },
        },
        Setting::EqBand {
            index: 1,
            update: EqBandUpdate {
                q: Some(0.01),
                ..EqBandUpdate::default()
            },
        },
    ] {
        let writes = device.transport().writes.len();
        assert!(device.apply(setting).is_err());
        assert_eq!(device.transport().writes.len(), writes);
    }
}

#[test]
fn dynamics_use_the_verified_layout_and_limits() {
    let config = baseline()
        .with_dynamics(DynamicsUpdate {
            compressor_threshold_db: Some(-20.0),
            compressor_ratio: Some(4.0),
            compressor_attack_ms: Some(20.0),
            compressor_release_ms: Some(200.0),
            compressor_makeup_gain_db: Some(8.0),
            expander_threshold_db: Some(-40.0),
            expander_ratio: Some(2.5),
            expander_attack_ms: Some(5.0),
            expander_release_ms: Some(300.0),
            expander_max_reduction_db: Some(30.0),
        })
        .unwrap();
    let raw = config.settings_raw();
    assert_eq!(raw[3], (-40_i8) as u8);
    assert_eq!(raw[4], 40);
    assert_eq!(&raw[5..9], &[5, 0, 44, 1]);
    assert_eq!(raw[9], 30);
    assert_eq!(raw[11], (-20_i8) as u8);
    assert_eq!(raw[12], 4);
    assert_eq!(&raw[13..17], &[20, 0, 200, 0]);
    assert_eq!(raw[17], 8);
    assert_eq!(raw[18..], SETTINGS[18..]);

    for update in [
        DynamicsUpdate {
            compressor_ratio: Some(0.0),
            ..DynamicsUpdate::default()
        },
        DynamicsUpdate {
            expander_attack_ms: Some(0.0),
            ..DynamicsUpdate::default()
        },
        DynamicsUpdate {
            expander_ratio: Some(4.0),
            ..DynamicsUpdate::default()
        },
    ] {
        assert!(baseline().with_dynamics(update).is_err());
    }
}

#[test]
fn noops_and_readback_validation_are_enforced() {
    let mut device = WaveDevice::new(FakeWave3Mk2Transport::new());
    device.apply(Setting::Mute(false)).unwrap();
    device.apply(Setting::Gain(34.0)).unwrap();
    device.apply(Setting::OutputVolume(-12.0)).unwrap();
    device.apply(Setting::MonitorMix(25.0)).unwrap();
    device.apply(Setting::Clipguard(true)).unwrap();
    assert!(device.transport().writes.is_empty());

    let mut transport = FakeWave3Mk2Transport::new();
    transport.corrupt_readback = true;
    let mut device = WaveDevice::new(transport);
    assert!(matches!(
        device.apply(Setting::Gain(30.0)),
        Err(DeviceError::ReadbackMismatch { .. })
    ));
}

#[test]
fn dsp_requires_the_measured_firmware() {
    let mut transport = FakeWave3Mk2Transport::new();
    transport.info = info([0, 2, 2]);
    let mut device = WaveDevice::new(transport);
    assert!(device.apply(Setting::EqEnabled(true)).is_err());
    assert!(device.transport().writes.is_empty());
    device.apply(Setting::Gain(30.0)).unwrap();
    assert_eq!(device.transport().writes.len(), 1);
}

#[test]
fn short_reads_fail_before_writes() {
    let mut transport = FakeWave3Mk2Transport::new();
    transport.short_settings = true;
    let mut device = WaveDevice::new(transport);
    assert!(device.read_config().is_err());
    assert!(device.apply(Setting::Gain(30.0)).is_err());
    assert!(device.transport().writes.is_empty());
}

#[test]
fn complete_blocks_restore_and_cross_model_restore_fails() {
    let mut device = WaveDevice::new(FakeWave3Mk2Transport::new());
    let updated = baseline()
        .with_gain(12.0)
        .unwrap()
        .with_output_volume(-30.0)
        .unwrap()
        .with_monitor_mix(80.0)
        .unwrap();
    let restored = device
        .write_config(&DeviceConfig::Wave3Mk2(updated.clone()))
        .unwrap();
    assert_eq!(restored, DeviceConfig::Wave3Mk2(updated));
    assert_eq!(device.transport().writes.len(), 3);

    let block16 = Config::from_bytes(&[0; 16]).unwrap();
    assert!(
        device
            .write_config(&DeviceConfig::Block16(block16))
            .is_err()
    );
    assert_eq!(device.transport().writes.len(), 3);
}

struct Block16Transport {
    model: DeviceModel,
    raw: [u8; 16],
    writes: usize,
}

impl Transport for Block16Transport {
    fn control_in(&mut self, value: u16, _length: usize) -> Result<Vec<u8>, DeviceError> {
        assert_eq!(value, CONFIG_VALUE);
        Ok(self.raw.to_vec())
    }

    fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        assert_eq!(value, CONFIG_VALUE);
        self.raw.copy_from_slice(raw);
        self.writes += 1;
        Ok(())
    }

    fn model(&self) -> DeviceModel {
        self.model
    }
}

#[test]
fn monitor_mix_dial_is_limited_to_wave3() {
    let raw = [
        0x00, 0x28, 0x00, 0xec, 0x00, 0x01, 0x01, 0x80, 0xe8, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
        0x01,
    ];
    for (model, succeeds) in [(DeviceModel::Wave3, true), (DeviceModel::Wave1, false)] {
        let transport = Block16Transport {
            model,
            raw,
            writes: 0,
        };
        let mut device = Wave1Device::new(transport);
        let result = device.set_dial_mode(DialMode::MonitorMix);
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(device.transport().writes, usize::from(succeeds));
        if succeeds {
            assert_eq!(device.transport().raw[12], 3);
            assert_eq!(
                Config::from_bytes(&device.transport().raw)
                    .unwrap()
                    .dial_mode(),
                Some(DialMode::MonitorMix)
            );
        }
    }
}

#[test]
fn product_ids_select_the_right_backend() {
    for (pid, model) in [
        (0x006e, DeviceModel::Wave1),
        (0x0070, DeviceModel::Wave3),
        (0x00ca, DeviceModel::Wave3Mk2),
    ] {
        assert_eq!(DeviceModel::from_product_id(pid), Some(model));
        assert_eq!(model.product_id(), pid);
    }
    assert_eq!(DeviceModel::from_product_id(0x00cb), None);
    assert_eq!(DeviceModel::from_product_id(0x0071), None);
}

#[test]
fn shared_driver_specs_match_each_models_capabilities() {
    let raw = [0; 16];
    let wave1 = WaveDevice::new(Block16Transport {
        model: DeviceModel::Wave1,
        raw,
        writes: 0,
    });
    assert!(std::ptr::eq(wave1.spec(), &WAVE1_SPEC));
    assert_eq!(wave1.spec().dial_modes.len(), 2);
    assert!(!wave1.spec().dial_modes.contains(&DialMode::MonitorMix));
    assert!(!wave1.spec().clipguard_modes);
    assert!(!wave1.spec().hardware_eq);

    let wave3 = WaveDevice::new(Block16Transport {
        model: DeviceModel::Wave3,
        raw,
        writes: 0,
    });
    assert!(std::ptr::eq(wave3.spec(), &WAVE3_SPEC));
    assert!(wave3.spec().dial_modes.contains(&DialMode::MonitorMix));
    assert!(!wave3.spec().clipguard_modes);
    assert!(!wave3.spec().hardware_eq);

    let wave3_mk2 = WaveDevice::new(FakeWave3Mk2Transport::new());
    assert!(std::ptr::eq(wave3_mk2.spec(), &WAVE3_MK2_SPEC));
    assert!(wave3_mk2.spec().dial_modes.is_empty());
    assert!(wave3_mk2.spec().clipguard_modes);
    assert!(wave3_mk2.spec().expander);
    assert!(wave3_mk2.spec().voice_tune);
    assert!(wave3_mk2.spec().compressor);
    assert!(wave3_mk2.spec().hardware_eq);
    assert!(wave3_mk2.spec().hardware_dynamics);
}
