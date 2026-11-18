use std::{
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use crate::{
    controller::{self, Controller, DeviceConnection, Event, Operation},
    demo,
    device::Transport,
};

mod types;
pub use types::*;

#[uniffi::export(rust, foreign)]
pub trait WaveLiteEventCallback: Send + Sync {
    fn on_event(&self, event: DeviceEvent);
}

trait ErasedController: Send {
    fn next_deadline(&self) -> Instant;
    fn next_operation(&self, now: Instant) -> Option<Operation>;
    fn execute(&mut self, operation: Operation) -> Option<Event>;
}

impl<T, F> ErasedController for Controller<T, F>
where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    fn next_deadline(&self) -> Instant {
        Controller::next_deadline(self)
    }

    fn next_operation(&self, now: Instant) -> Option<Operation> {
        Controller::next_operation(self, now)
    }

    fn execute(&mut self, operation: Operation) -> Option<Event> {
        Controller::execute(self, operation)
    }
}

#[derive(uniffi::Object)]
pub struct WaveLiteController {
    core: Mutex<Box<dyn ErasedController>>,
    callback: Arc<dyn WaveLiteEventCallback>,
}

impl WaveLiteController {
    fn new(
        core: impl ErasedController + 'static,
        callback: Arc<dyn WaveLiteEventCallback>,
    ) -> Arc<Self> {
        Arc::new(Self {
            core: Mutex::new(Box::new(core)),
            callback,
        })
    }

    fn lock_core(&self) -> MutexGuard<'_, Box<dyn ErasedController>> {
        self.core.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn emit(&self, event: Event) {
        self.callback.on_event(event.into());
    }
}

#[uniffi::export]
impl WaveLiteController {
    #[uniffi::constructor]
    pub fn usb(callback: Arc<dyn WaveLiteEventCallback>) -> Arc<Self> {
        Self::new(controller::usb(), callback)
    }

    #[uniffi::constructor]
    pub fn demo(
        model: DemoModel,
        delay_ms: u64,
        callback: Arc<dyn WaveLiteEventCallback>,
    ) -> Arc<Self> {
        Self::new(
            demo::controller(model.into(), Duration::from_millis(delay_ms)),
            callback,
        )
    }

    pub fn next_delay_ms(&self) -> u64 {
        self.lock_core()
            .next_deadline()
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(u128::from(u64::MAX)) as u64
    }

    pub fn execute_next(&self) -> bool {
        let mut core = self.lock_core();
        let Some(operation) = core.next_operation(Instant::now()) else {
            return false;
        };
        let event = core.execute(operation);
        drop(core);
        if let Some(event) = event {
            self.emit(event);
        }
        true
    }

    pub fn apply(&self, setting: Setting) {
        let event = self.lock_core().execute(Operation::Set(setting.into()));
        if let Some(event) = event {
            self.emit(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct EventCollector(Mutex<Vec<DeviceEvent>>);

    impl WaveLiteEventCallback for EventCollector {
        fn on_event(&self, event: DeviceEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[test]
    fn fake_controller_emits_typed_events_and_applies_settings() {
        let events = Arc::new(EventCollector::default());
        let controller = WaveLiteController::demo(DemoModel::Wave3Mk2, 0, events.clone());

        assert!(controller.execute_next());
        controller.apply(Setting::Gain(12.0));
        drop(controller);

        let events = events.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        match &events[0] {
            DeviceEvent::Connected { model, info, .. } => {
                assert_eq!(*model, DeviceModel::Wave3Mk2);
                assert_eq!(info.serial, "W3MK2DEMO002");
            }
            event => panic!("expected connected event, got {event:?}"),
        }
        match &events[1] {
            DeviceEvent::ConfigChanged(config) => assert_eq!(config.gain_db, 12.0),
            event => panic!("expected config-changed event, got {event:?}"),
        }
    }
}
