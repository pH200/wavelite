use std::{
    collections::VecDeque,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel},
    },
    time::Instant,
};

use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_APP},
};
use windows_future::IAsyncAction;

use crate::{
    controller::{Command, Controller, DeviceConnection, Event, Operation},
    device::{DeviceConfig, DeviceError, Transport, WaveDevice},
    protocol::DeviceInfo,
};

pub const DEVICE_EVENT_MESSAGE: u32 = WM_APP + 73;

pub struct EventQueue {
    events: Mutex<VecDeque<Event>>,
    closing: AtomicBool,
}

impl EventQueue {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(VecDeque::new()),
            closing: AtomicBool::new(false),
        })
    }

    pub fn drain(&self) -> VecDeque<Event> {
        std::mem::take(
            &mut *self
                .events
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }

    fn push(&self, event: Event) {
        if self.closing.load(Ordering::Acquire) {
            return;
        }
        self.events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push_back(event);
    }

    fn close(&self) {
        self.closing.store(true, Ordering::Release);
    }
}

struct CommandChannel {
    sender: Sender<Command>,
}

impl CommandChannel {
    fn send(&self, command: Command) {
        let _ = self.sender.send(command);
    }
}

#[derive(Clone, Copy)]
struct WindowHandle(HWND);

unsafe impl Send for WindowHandle {}

pub struct Worker {
    action: IAsyncAction,
    commands: CommandChannel,
}

impl Worker {
    pub fn spawn<T, F>(
        hwnd: HWND,
        controller: Controller<T, F>,
        events: Arc<EventQueue>,
    ) -> io::Result<Self>
    where
        T: Transport + Send + 'static,
        F: FnMut() -> Result<(WaveDevice<T>, DeviceInfo, DeviceConfig), DeviceError>
            + Send
            + 'static,
    {
        let (sender, receiver) = channel();
        let commands = CommandChannel { sender };
        let hwnd = WindowHandle(hwnd);
        let action = IAsyncAction::spawn(move || {
            worker_main(WorkerArgs {
                hwnd,
                controller,
                events,
                receiver,
            });
            Ok(())
        });
        Ok(Self { action, commands })
    }

    pub fn send(&self, command: Command) {
        self.commands.send(command);
    }

    pub fn shutdown(self, events: &EventQueue) {
        events.close();
        self.commands.send(Command::Shutdown);
        if let Err(error) = self.action.join() {
            eprintln!("The Windows device worker failed during shutdown: {error}");
        }
    }
}

struct WorkerArgs<T, F> {
    hwnd: WindowHandle,
    controller: Controller<T, F>,
    events: Arc<EventQueue>,
    receiver: Receiver<Command>,
}

fn worker_main<T, F>(mut args: WorkerArgs<T, F>)
where
    T: Transport + Send + 'static,
    F: FnMut() -> Result<(WaveDevice<T>, DeviceInfo, DeviceConfig), DeviceError> + Send + 'static,
{
    let WorkerArgs {
        hwnd,
        controller,
        events,
        receiver,
    } = &mut args;
    loop {
        let timeout = controller
            .next_deadline()
            .saturating_duration_since(Instant::now());
        let timed_out = match receiver.recv_timeout(timeout) {
            Ok(Command::Set(setting)) => {
                execute(controller, Operation::Set(setting), events, *hwnd);
                false
            }
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => true,
        };

        loop {
            match receiver.try_recv() {
                Ok(Command::Set(setting)) => {
                    execute(controller, Operation::Set(setting), events, *hwnd);
                }
                Ok(Command::Shutdown) | Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => break,
            }
        }

        if timed_out && let Some(operation) = controller.next_operation(Instant::now()) {
            execute(controller, operation, events, *hwnd);
        }
    }
}

fn execute<T, F>(
    controller: &mut Controller<T, F>,
    operation: Operation,
    events: &EventQueue,
    hwnd: WindowHandle,
) where
    T: Transport + 'static,
    F: FnMut() -> DeviceConnection<T>,
{
    if let Some(event) = controller.execute(operation) {
        events.push(event);
        post_event(hwnd);
    }
}

fn post_event(hwnd: WindowHandle) {
    let _ = unsafe { PostMessageW(Some(hwnd.0), DEVICE_EVENT_MESSAGE, WPARAM(0), LPARAM(0)) };
}
