use std::time::Duration;

use crate::{
    controller::{Controller, DeviceConnection},
    device::{
        CONFIG_VALUE, DeviceError, DeviceModel, INFO_VALUE, METER_VALUE, Transport, WaveDevice,
    },
    protocol::ProtocolError,
    wave3_mk2::{
        self, HEADPHONES_VALUE, INFO_VALUE as WAVE3_MK2_INFO_VALUE, MONITOR_VALUE, SETTINGS_VALUE,
        Wave3Mk2Config,
    },
};

pub fn controller(
    model: DeviceModel,
    delay: Duration,
) -> Controller<DemoTransport, impl FnMut() -> DeviceConnection<DemoTransport>> {
    Controller::new(move || {
        let mut device = WaveDevice::new(DemoTransport::new(model, delay));
        let info = device.read_info()?;
        let config = device.read_config()?;
        Ok((device, info, config))
    })
}

pub struct DemoTransport {
    model: DeviceModel,
    delay: Duration,
    block16_raw: [u8; 16],
    block16_info: Vec<u8>,
    wave3_mk2_config: Option<Wave3Mk2Config>,
    wave3_mk2_info: Vec<u8>,
}

impl DemoTransport {
    fn new(model: DeviceModel, delay: Duration) -> Self {
        let serial = match model {
            DeviceModel::Wave3 => b"W3MK1DEMO001",
            DeviceModel::Wave3Mk2 => b"W3MK2DEMO002",
            DeviceModel::Wave1 => unreachable!(),
        };
        let mut block16_info = vec![0; 64];
        block16_info[0..2].copy_from_slice(&[5, 3]);
        block16_info[21..24].copy_from_slice(&[1, 2, 4]);
        block16_info[36..48].copy_from_slice(serial);

        let mut wave3_mk2_info = vec![0; wave3_mk2::INFO_LENGTH];
        wave3_mk2_info[0..5].copy_from_slice(&[1, 5, 2, 2, 0]);
        wave3_mk2_info[5..8].copy_from_slice(&[0, 2, 3]);
        wave3_mk2_info[35..47].copy_from_slice(serial);

        let wave3_mk2_config = (model == DeviceModel::Wave3Mk2).then(|| {
            Wave3Mk2Config::from_blocks(&SETTINGS, &[0x30, 0], &[0x64, 1, 2, 3, 2, 0])
                .expect("valid MK.2 demo config")
        });
        Self {
            model,
            delay,
            block16_raw: [
                0x00, 0x18, 0x00, 0xec, 0x00, 0x01, 0x01, 0x80, 0xe8, 0x00, 0x00, 0x32, 0x03, 0x00,
                0x00, 0x01,
            ],
            block16_info,
            wave3_mk2_config,
            wave3_mk2_info,
        }
    }

    fn wave3_mk2(&self) -> &Wave3Mk2Config {
        self.wave3_mk2_config.as_ref().expect("MK.2 demo config")
    }

    fn pause(&self) {
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
    }
}

const SETTINGS: [u8; 38] = [
    0x1c, 0xc2, 0x00, 0xd4, 0x20, 0x0a, 0x00, 0xc8, 0x00, 0x14, 0x28, 0xf4, 0x04, 0x0a, 0x00, 0x64,
    0x00, 0x06, 0x05, 0x0c, 0xe8, 0x03, 0x00, 0x03, 0x0d, 0xe3, 0x00, 0x00, 0x05, 0xf4, 0x5c, 0x03,
    0xf2, 0x25, 0x07, 0xfd, 0x27, 0xe8,
];

impl Transport for DemoTransport {
    fn control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
        self.pause();
        match value {
            CONFIG_VALUE => Ok(self.block16_raw.to_vec()),
            INFO_VALUE => Ok(self.block16_info.clone()),
            METER_VALUE => Ok(vec![0; length]),
            _ => panic!("unexpected 16-byte read selector {value}"),
        }
    }

    fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        self.pause();
        assert_eq!(value, CONFIG_VALUE);
        self.block16_raw.copy_from_slice(raw);
        Ok(())
    }

    fn wave3_mk2_control_in(&mut self, value: u16, _length: usize) -> Result<Vec<u8>, DeviceError> {
        self.pause();
        match value {
            SETTINGS_VALUE => Ok(self.wave3_mk2().settings_raw().to_vec()),
            HEADPHONES_VALUE => Ok(self.wave3_mk2().headphones_raw().to_vec()),
            MONITOR_VALUE => Ok(self.wave3_mk2().monitor_raw().to_vec()),
            WAVE3_MK2_INFO_VALUE => Ok(self.wave3_mk2_info.clone()),
            _ => panic!("unexpected MK.2 read selector {value}"),
        }
    }

    fn wave3_mk2_control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
        self.pause();
        let current = self.wave3_mk2().clone();
        let updated = match value {
            SETTINGS_VALUE => {
                Wave3Mk2Config::from_blocks(raw, current.headphones_raw(), current.monitor_raw())
            }
            HEADPHONES_VALUE => {
                Wave3Mk2Config::from_blocks(current.settings_raw(), raw, current.monitor_raw())
            }
            MONITOR_VALUE => {
                Wave3Mk2Config::from_blocks(current.settings_raw(), current.headphones_raw(), raw)
            }
            _ => return Err(ProtocolError::InvalidValue("invalid MK.2 write selector").into()),
        }?;
        self.wave3_mk2_config = Some(updated);
        Ok(())
    }

    fn model(&self) -> DeviceModel {
        self.model
    }
}
