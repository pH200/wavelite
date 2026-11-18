use wavelite::{
    device::{CONFIG_VALUE, DeviceError, INFO_VALUE, METER_VALUE, Transport, Wave1Device},
    protocol::{Config, DeviceInfo, DialMode, ProtocolError},
};

const BASELINE: [u8; 16] = [
    0x00, 0x28, 0x00, 0xec, 0x00, 0x01, 0x01, 0x80, 0xe8, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x01,
];

fn baseline() -> Config {
    Config::from_bytes(&BASELINE).unwrap()
}

fn info() -> [u8; 64] {
    let mut info = [0; 64];
    info[0..2].copy_from_slice(&[5, 3]);
    info[21..24].copy_from_slice(&[1, 2, 2]);
    info[36..48].copy_from_slice(b"N8Q3V7K2X9L4");
    info
}

struct FakeTransport {
    raw: [u8; 16],
    writes: Vec<(u16, Vec<u8>)>,
    corrupt_readback: bool,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            raw: BASELINE,
            writes: Vec::new(),
            corrupt_readback: false,
        }
    }
}

impl Transport for FakeTransport {
    fn control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
        match value {
            CONFIG_VALUE => {
                let mut raw = self.raw;
                if self.corrupt_readback && !self.writes.is_empty() {
                    self.corrupt_readback = false;
                    raw[15] ^= 1;
                }
                Ok(raw.to_vec())
            }
            METER_VALUE => Ok((0..length).map(|value| value as u8).collect()),
            INFO_VALUE => Ok(info()[..length].to_vec()),
            _ => panic!("unexpected read selector {value}"),
        }
    }

    fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        self.writes.push((value, raw.to_vec()));
        self.raw.copy_from_slice(raw);
        Ok(())
    }
}

#[test]
fn decodes_windows_reference() {
    let config = baseline();
    assert_eq!(config.gain_db(), 40.0);
    assert!(config.clipguard());
    assert_eq!(config.output_volume_db(), -23.5);
    assert_eq!(config.dial_mode(), Some(DialMode::OutputVolume));
}

#[test]
fn rejects_wrong_length() {
    assert_eq!(
        Config::from_bytes(&[0; 15]).unwrap_err(),
        ProtocolError::InvalidConfigLength
    );
    assert_eq!(
        DeviceInfo::from_bytes(&[0; 63]).unwrap_err(),
        ProtocolError::InvalidInfoLength
    );
}

#[test]
fn changes_only_dial_byte() {
    let updated = baseline().with_dial_mode(DialMode::InputGain);
    assert_eq!(updated.raw()[12], 1);
    assert_eq!(updated.raw()[..12], BASELINE[..12]);
    assert_eq!(updated.raw()[13..], BASELINE[13..]);
}

#[test]
fn changes_only_clipguard_byte() {
    let updated = baseline().with_clipguard(false);
    assert_eq!(updated.raw()[5], 0);
    assert_eq!(updated.raw()[..5], BASELINE[..5]);
    assert_eq!(updated.raw()[6..], BASELINE[6..]);
}

#[test]
fn changes_only_lowcut_byte() {
    let updated = baseline().with_lowcut(false);
    assert_eq!(updated.raw()[6], 0);
    assert_eq!(updated.raw()[..6], BASELINE[..6]);
    assert_eq!(updated.raw()[7..], BASELINE[7..]);
}

#[test]
fn gain_uses_q8_8_and_limits() {
    assert_eq!(baseline().with_gain(12.5).unwrap().gain_raw(), 3200);
    for value in [-0.1, 40.1, f64::NAN, f64::INFINITY] {
        assert!(baseline().with_gain(value).is_err());
    }
}

#[test]
fn output_volume_uses_signed_q8_8() {
    let config = baseline().with_output_volume(-20.0).unwrap();
    assert_eq!(config.output_volume_db(), -20.0);
    assert_eq!(config.raw()[7..9], [0x00, 0xec]);
}

#[test]
fn monitor_mix_uses_q8_8() {
    assert_eq!(
        baseline().with_monitor_mix(100.0).unwrap().raw()[10..12],
        [0x00, 0x64]
    );
}

#[test]
fn dial_write_and_readback() {
    let mut device = Wave1Device::new(FakeTransport::new());
    let config = device.set_dial_mode(DialMode::InputGain).unwrap();
    assert_eq!(config.dial_mode(), Some(DialMode::InputGain));
    let transport = &device.transport();
    assert_eq!(transport.writes, [(CONFIG_VALUE, config.raw().to_vec())]);
}

#[test]
fn clipguard_write_and_restore() {
    let mut device = Wave1Device::new(FakeTransport::new());
    let off = device.set_clipguard(false).unwrap();
    let on = device.set_clipguard(true).unwrap();
    assert!(!off.clipguard());
    assert_eq!(on.raw(), &BASELINE);
    assert_eq!(device.transport().writes.len(), 2);
}

#[test]
fn noop_does_not_write() {
    let mut device = Wave1Device::new(FakeTransport::new());
    device.set_clipguard(true).unwrap();
    assert!(device.transport().writes.is_empty());
}

#[test]
fn readback_mismatch_fails() {
    let mut transport = FakeTransport::new();
    transport.corrupt_readback = true;
    let mut device = Wave1Device::new(transport);
    assert!(matches!(
        device.set_dial_mode(DialMode::InputGain),
        Err(DeviceError::ReadbackMismatch { .. })
    ));
}

#[test]
fn info_and_meter() {
    let mut device = Wave1Device::new(FakeTransport::new());
    let info = device.read_info().unwrap();
    assert_eq!(info.api_version(), "5.3");
    assert_eq!(info.firmware_version(), "1.2.2");
    assert_eq!(info.serial(), "N8Q3V7K2X9L4");
    assert_eq!(device.read_meter().unwrap(), (0..8).collect::<Vec<_>>());
}
