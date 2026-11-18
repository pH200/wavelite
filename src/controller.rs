use std::time::{Duration, Instant};

use crate::{
    device::{DeviceConfig, DeviceError, DeviceModel, Transport, UsbTransport, WaveDevice},
    protocol::DeviceInfo,
};

pub use crate::device::Setting;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub enum Command {
    Set(Setting),
    Shutdown,
}

#[derive(Debug)]
pub enum Event {
    Connected {
        model: DeviceModel,
        info: DeviceInfo,
        config: DeviceConfig,
    },
    ConfigChanged(DeviceConfig),
    Refreshed(DeviceConfig),
    Disconnected(String),
    OperationFailed(String),
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Connect,
    Poll,
    Set(Setting),
}

pub struct Controller<T, F> {
    device: Option<WaveDevice<T>>,
    connector: F,
    last_config: Option<DeviceConfig>,
    last_disconnect: Option<String>,
    next_connect: Instant,
    next_poll: Instant,
}

pub type DeviceConnection<T> = Result<(WaveDevice<T>, DeviceInfo, DeviceConfig), DeviceError>;
pub type UsbController = Controller<UsbTransport, fn() -> DeviceConnection<UsbTransport>>;

pub fn usb() -> UsbController {
    Controller::new(connect)
}

impl<T, F> Controller<T, F>
where
    T: Transport + 'static,
    F: FnMut() -> DeviceConnection<T>,
{
    pub fn new(connector: F) -> Self {
        Self {
            device: None,
            connector,
            last_config: None,
            last_disconnect: None,
            next_connect: Instant::now(),
            next_poll: Instant::now(),
        }
    }

    pub fn next_operation(&self, now: Instant) -> Option<Operation> {
        if self.device.is_none() && now >= self.next_connect {
            Some(Operation::Connect)
        } else if self.device.is_some() && now >= self.next_poll {
            Some(Operation::Poll)
        } else {
            None
        }
    }

    pub fn next_deadline(&self) -> Instant {
        if self.device.is_some() {
            self.next_poll
        } else {
            self.next_connect
        }
    }

    pub fn execute(&mut self, operation: Operation) -> Option<Event> {
        match operation {
            Operation::Connect => self.connect(),
            Operation::Poll => self.poll(),
            Operation::Set(setting) => Some(self.apply(setting)),
        }
    }

    fn connect(&mut self) -> Option<Event> {
        if self.device.is_some() {
            return None;
        }
        match (self.connector)() {
            Ok((connected, info, config)) => {
                self.last_disconnect = None;
                self.last_config = Some(config.clone());
                let model = connected.model();
                self.device = Some(connected);
                self.next_poll = Instant::now() + POLL_INTERVAL;
                Some(Event::Connected {
                    model,
                    info,
                    config,
                })
            }
            Err(error) => {
                let message = error.to_string();
                let event = (self.last_disconnect.as_ref() != Some(&message))
                    .then(|| Event::Disconnected(message.clone()));
                self.last_disconnect = Some(message);
                self.next_connect = Instant::now() + RECONNECT_INTERVAL;
                event
            }
        }
    }

    fn poll(&mut self) -> Option<Event> {
        let active = self.device.as_mut()?;
        let now = Instant::now();
        let event = match active.read_config() {
            Ok(config) => {
                let event = (self.last_config.as_ref() != Some(&config))
                    .then(|| Event::Refreshed(config.clone()));
                self.last_config = Some(config);
                event
            }
            Err(error) => {
                let message = error.to_string();
                self.device = None;
                self.last_config = None;
                self.last_disconnect = Some(message.clone());
                self.next_connect = now + RECONNECT_INTERVAL;
                Some(Event::Disconnected(message))
            }
        };
        self.next_poll = now + POLL_INTERVAL;
        event
    }

    fn apply(&mut self, setting: Setting) -> Event {
        let Some(active) = self.device.as_mut() else {
            return Event::OperationFailed(
                "No supported Elgato Wave microphone is connected".to_owned(),
            );
        };
        match active.apply(setting) {
            Ok(config) => {
                self.last_config = Some(config.clone());
                Event::ConfigChanged(config)
            }
            Err(error) => {
                self.device = None;
                self.last_config = None;
                self.next_connect = Instant::now() + RECONNECT_INTERVAL;
                Event::OperationFailed(error.to_string())
            }
        }
    }
}

fn connect() -> Result<(WaveDevice<UsbTransport>, DeviceInfo, DeviceConfig), DeviceError> {
    let mut device = WaveDevice::new(UsbTransport::open(None)?);
    let info = device.read_info()?;
    let config = device.read_config()?;
    Ok((device, info, config))
}

#[cfg(test)]
fn apply<T: crate::device::Transport + 'static>(
    device: &mut WaveDevice<T>,
    setting: Setting,
) -> Result<DeviceConfig, DeviceError> {
    device.apply(setting)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        device::{CONFIG_VALUE, Transport},
        protocol::{Config, DialMode},
    };

    const BASELINE: [u8; 16] = [
        0x00, 0x28, 0x00, 0xec, 0x00, 0x01, 0x01, 0x80, 0xe8, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
        0x01,
    ];

    struct FakeTransport([u8; 16]);

    impl Transport for FakeTransport {
        fn control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
            assert_eq!(value, CONFIG_VALUE);
            assert_eq!(length, 16);
            Ok(self.0.to_vec())
        }

        fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
            assert_eq!(value, CONFIG_VALUE);
            self.0.copy_from_slice(raw);
            Ok(())
        }
    }

    #[test]
    fn connection_errors_are_deduplicated_and_backed_off() {
        let mut controller = Controller::new(
            || -> Result<(WaveDevice<FakeTransport>, DeviceInfo, DeviceConfig), DeviceError> {
                Err(DeviceError::NotFound)
            },
        );
        assert!(matches!(
            controller.next_operation(Instant::now()),
            Some(Operation::Connect)
        ));
        assert!(matches!(
            controller.execute(Operation::Connect),
            Some(Event::Disconnected(_))
        ));
        assert!(controller.next_operation(Instant::now()).is_none());
        assert!(controller.execute(Operation::Connect).is_none());
        assert!(controller.next_deadline() > Instant::now());
    }

    #[test]
    fn apply_dispatches_all_shared_settings() {
        let baseline = || Config::from_bytes(&BASELINE).unwrap();
        let cases = [
            (Setting::Gain(12.5), baseline().with_gain(12.5).unwrap()),
            (Setting::Mute(true), baseline().with_mute(true)),
            (Setting::Clipguard(false), baseline().with_clipguard(false)),
            (Setting::Lowcut(false), baseline().with_lowcut(false)),
            (
                Setting::OutputVolume(-20.0),
                baseline().with_output_volume(-20.0).unwrap(),
            ),
            (
                Setting::MonitorMix(75.0),
                baseline().with_monitor_mix(75.0).unwrap(),
            ),
            (
                Setting::DialMode(DialMode::InputGain),
                baseline().with_dial_mode(DialMode::InputGain),
            ),
        ];

        for (setting, expected) in cases {
            let mut device = WaveDevice::new(FakeTransport(BASELINE));
            assert_eq!(
                apply(&mut device, setting).unwrap(),
                DeviceConfig::Block16(expected)
            );
        }
    }
}
