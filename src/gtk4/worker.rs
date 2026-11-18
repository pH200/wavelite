use std::time::Instant;

use crate::{
    controller::{Command, Controller, Event, Operation},
    device::{DeviceConfig, DeviceError, Transport, WaveDevice},
    protocol::DeviceInfo,
};
use async_channel::Receiver;
use futures_util::future::{self, Either, FutureExt};
use gtk::{gio, glib};

pub fn spawn_with<T, F>(
    commands: Receiver<Command>,
    controller: Controller<T, F>,
    on_event: impl FnMut(Event) + 'static,
) where
    T: Transport + Send + 'static,
    F: FnMut() -> Result<(WaveDevice<T>, DeviceInfo, DeviceConfig), DeviceError> + Send + 'static,
{
    glib::spawn_future_local(run(commands, controller, on_event));
}

async fn run<T, F>(
    commands: Receiver<Command>,
    mut controller: Controller<T, F>,
    mut on_event: impl FnMut(Event),
) where
    T: Transport + Send + 'static,
    F: FnMut() -> Result<(WaveDevice<T>, DeviceInfo, DeviceConfig), DeviceError> + Send + 'static,
{
    loop {
        let delay = controller
            .next_deadline()
            .saturating_duration_since(Instant::now());
        let command = commands.recv().fuse();
        let deadline = glib::timeout_future(delay).fuse();
        futures_util::pin_mut!(command, deadline);
        let operation = match future::select(command, deadline).await {
            Either::Left((Ok(Command::Set(setting)), _)) => Operation::Set(setting),
            Either::Left((Ok(Command::Shutdown), _)) | Either::Left((Err(_), _)) => break,
            Either::Right(((), _)) => {
                let Some(operation) = controller.next_operation(Instant::now()) else {
                    continue;
                };
                operation
            }
        };

        match gio::spawn_blocking(move || {
            let event = controller.execute(operation);
            (controller, event)
        })
        .await
        {
            Ok((returned, event)) => {
                controller = returned;
                if let Some(event) = event {
                    on_event(event);
                }
            }
            Err(_) => {
                on_event(Event::Disconnected(
                    "The device worker stopped unexpectedly. Restart WaveLite to reconnect."
                        .to_owned(),
                ));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    use crate::{
        controller::{Command, Controller, Event, Setting},
        device::{CONFIG_VALUE, DeviceError, Transport, WaveDevice},
    };
    use gtk::glib::{self, ControlFlow};

    use super::spawn_with;

    static MAIN_LOOP_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct SlowTransport {
        raw: [u8; 16],
        reads: Arc<AtomicUsize>,
    }

    impl Transport for SlowTransport {
        fn control_in(&mut self, value: u16, length: usize) -> Result<Vec<u8>, DeviceError> {
            if value == CONFIG_VALUE {
                self.reads.fetch_add(1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(150));
                Ok(self.raw.to_vec())
            } else {
                Ok(vec![0; length])
            }
        }

        fn control_out(&mut self, value: u16, raw: &[u8]) -> Result<(), DeviceError> {
            assert_eq!(value, CONFIG_VALUE);
            thread::sleep(Duration::from_millis(150));
            self.raw.copy_from_slice(raw);
            Ok(())
        }
    }

    #[test]
    fn main_loop_remains_responsive_with_scheduled_usb_polling() {
        let _guard = MAIN_LOOP_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (sender, commands) = async_channel::unbounded();
        let reads = Arc::new(AtomicUsize::new(0));
        let connected = Rc::new(Cell::new(false));
        let applied = Rc::new(Cell::new(false));
        let polled = Rc::new(Cell::new(false));
        let finished = Rc::new(Cell::new(false));
        let ticks = Rc::new(Cell::new(0));
        let main_loop = glib::MainLoop::new(None, false);
        spawn_with(
            commands,
            Controller::new({
                let reads = reads.clone();
                move || {
                    let mut device = WaveDevice::new(SlowTransport {
                        raw: [0; 16],
                        reads: reads.clone(),
                    });
                    let info = device.read_info()?;
                    let config = device.read_config()?;
                    Ok((device, info, config))
                }
            }),
            {
                let connected = connected.clone();
                let applied = applied.clone();
                let polled = polled.clone();
                let finished = finished.clone();
                let reads = reads.clone();
                move |event| match event {
                    Event::Connected { .. } => {
                        connected.set(true);
                        let idle_reads = reads.load(Ordering::SeqCst);
                        glib::timeout_add_local_once(Duration::from_millis(250), {
                            let reads = reads.clone();
                            move || {
                                assert_eq!(
                                    reads.load(Ordering::SeqCst),
                                    idle_reads,
                                    "configuration poll happened before 500 ms"
                                );
                            }
                        });
                        glib::timeout_add_local_once(Duration::from_millis(650), {
                            let sender = sender.clone();
                            let reads = reads.clone();
                            move || {
                                assert_eq!(
                                    reads.load(Ordering::SeqCst),
                                    idle_reads + 1,
                                    "500 ms configuration poll did not happen exactly once"
                                );
                                sender.try_send(Command::Set(Setting::Gain(12.5))).unwrap();
                                sender.try_send(Command::Set(Setting::Mute(true))).unwrap();
                            }
                        });
                    }
                    Event::ConfigChanged(config) if config.gain_db() == 12.5 && config.muted() => {
                        applied.set(true);
                        let before_poll = reads.load(Ordering::SeqCst);
                        glib::timeout_add_local_once(Duration::from_millis(250), {
                            let sender = sender.clone();
                            let reads = reads.clone();
                            let polled = polled.clone();
                            let finished = finished.clone();
                            move || {
                                assert_eq!(
                                    reads.load(Ordering::SeqCst),
                                    before_poll + 1,
                                    "scheduled poll must run after queued writes"
                                );
                                polled.set(true);
                                sender.try_send(Command::Shutdown).unwrap();
                                glib::timeout_add_local_once(Duration::from_millis(75), {
                                    let finished = finished.clone();
                                    move || finished.set(true)
                                });
                            }
                        });
                    }
                    _ => {}
                }
            },
        );
        glib::timeout_add_local(Duration::from_millis(5), {
            let main_loop = main_loop.clone();
            let finished = finished.clone();
            let ticks = ticks.clone();
            move || {
                ticks.set(ticks.get() + 1);
                if finished.get() {
                    main_loop.quit();
                    ControlFlow::Break
                } else {
                    ControlFlow::Continue
                }
            }
        });
        let watchdog = glib::timeout_add_local_once(Duration::from_secs(5), {
            let main_loop = main_loop.clone();
            move || main_loop.quit()
        });
        main_loop.run();
        watchdog.remove();
        assert!(connected.get(), "device did not connect");
        assert!(applied.get(), "settings were not applied");
        assert!(polled.get(), "scheduled poll did not finish");
        assert!(
            ticks.get() >= 10,
            "GTK timers were blocked by USB I/O: {} ticks",
            ticks.get()
        );
    }

    #[test]
    fn reconnects_after_one_second() {
        let _guard = MAIN_LOOP_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (sender, commands) = async_channel::unbounded();
        let attempts = Arc::new(AtomicUsize::new(0));
        let main_loop = glib::MainLoop::new(None, false);
        spawn_with(
            commands,
            Controller::new({
                let attempts = attempts.clone();
                move || -> Result<
                    (
                        WaveDevice<SlowTransport>,
                        crate::protocol::DeviceInfo,
                        crate::device::DeviceConfig,
                    ),
                    DeviceError,
                > {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(DeviceError::NotFound)
                }
            }),
            move |_| {},
        );
        glib::timeout_add_local_once(Duration::from_millis(500), {
            let attempts = attempts.clone();
            move || {
                assert_eq!(
                    attempts.load(Ordering::SeqCst),
                    1,
                    "reconnection happened before one second"
                );
            }
        });
        glib::timeout_add_local_once(Duration::from_millis(1300), {
            let attempts = attempts.clone();
            let main_loop = main_loop.clone();
            move || {
                assert_eq!(
                    attempts.load(Ordering::SeqCst),
                    2,
                    "microphone was not reconnected after one second"
                );
                sender.try_send(Command::Shutdown).unwrap();
                glib::timeout_add_local_once(Duration::from_millis(75), {
                    let main_loop = main_loop.clone();
                    move || main_loop.quit()
                });
            }
        });
        let watchdog = glib::timeout_add_local_once(Duration::from_secs(5), {
            let main_loop = main_loop.clone();
            move || main_loop.quit()
        });
        main_loop.run();
        watchdog.remove();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
