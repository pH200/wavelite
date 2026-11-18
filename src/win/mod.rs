mod dsp;
mod screenshot;
mod worker;

use std::{
    cell::{Cell, RefCell},
    error::Error,
    ops::Deref,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use native_windows_gui as nwg;
use nwg::NativeUi;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

use crate::{
    controller::{Command, Controller, DeviceConnection, Event, Setting},
    device::{DeviceConfig, DeviceModel, Transport},
    protocol::DialMode,
    wave3_mk2::{ClipguardMode, DynamicsUpdate, EqBandType, EqBandUpdate, Wave3Mk2Config},
};
use worker::{DEVICE_EVENT_MESSAGE, EventQueue, Worker};

const APP_ICON_ID: usize = 1;
const RAW_HANDLER_ID: usize = 0x10001;
const MAIN_WINDOW_WIDTH: i32 = 420;
const MAIN_CONTENT_WIDTH: i32 = 398;
const MAIN_WINDOW_HEIGHT: i32 = 481;
const MAIN_WINDOW_STATUS_HEIGHT: i32 = 520;

#[derive(Default)]
pub struct Options {
    pub screenshot: Option<PathBuf>,
    pub responsiveness_test: bool,
    pub wave_fx_page: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestPhase {
    Connecting,
    Applying(usize),
    Done,
}

struct NativeApp<T, F> {
    window: nwg::Window,
    title_font: nwg::Font,
    heading_font: nwg::Font,
    headings: Vec<nwg::Label>,
    dial_heading: nwg::Label,
    wave_fx_heading: nwg::Label,
    device_name: nwg::Label,
    firmware_label: nwg::Label,
    serial_label: nwg::Label,
    reveal_button: nwg::Button,
    gain_label: nwg::Label,
    gain_value: nwg::Label,
    gain: nwg::TrackBar,
    mute: nwg::CheckBox,
    clipguard: nwg::CheckBox,
    lowcut: nwg::CheckBox,
    output_label: nwg::Label,
    output_value: nwg::Label,
    output: nwg::TrackBar,
    mix_label: nwg::Label,
    mix_value: nwg::Label,
    mix: nwg::TrackBar,
    dial_label: nwg::Label,
    dial: nwg::ComboBox<String>,
    dsp_button: nwg::Button,
    dsp_window: nwg::Window,
    dsp: dsp::DspControls,
    status: nwg::Label,
    capture_timer: nwg::AnimationTimer,
    test_timer: nwg::AnimationTimer,
    controller: RefCell<Option<Controller<T, F>>>,
    worker: RefCell<Option<Worker>>,
    events: Arc<EventQueue>,
    options: Options,
    model: Cell<Option<DeviceModel>>,
    serial: RefCell<String>,
    serial_visible: Cell<bool>,
    syncing: Cell<bool>,
    pending: Cell<usize>,
    test_phase: Cell<TestPhase>,
    test_ticks: Cell<usize>,
    test_mark: Cell<usize>,
    result: RefCell<Option<Result<(), String>>>,
    large_icon: nwg::Icon,
    small_icon: nwg::Icon,
}

struct NativeAppUi<T, F> {
    inner: Rc<NativeApp<T, F>>,
    default_handler: RefCell<Option<nwg::EventHandler>>,
    dsp_handler: RefCell<Option<nwg::EventHandler>>,
    raw_handler: RefCell<Option<nwg::RawEventHandler>>,
}

impl<T, F> Deref for NativeAppUi<T, F> {
    type Target = NativeApp<T, F>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, F> Drop for NativeAppUi<T, F> {
    fn drop(&mut self) {
        self.inner.shutdown_worker();
        if let Some(handler) = self.dsp_handler.borrow_mut().take() {
            nwg::unbind_event_handler(&handler);
        }
        if let Some(handler) = self.raw_handler.borrow_mut().take() {
            let _ = nwg::unbind_raw_event_handler(&handler);
        }
        if let Some(handler) = self.default_handler.borrow_mut().take() {
            nwg::unbind_event_handler(&handler);
        }
    }
}

impl<T, F> NativeApp<T, F> {
    fn new(controller: Controller<T, F>, options: Options) -> Self {
        Self {
            window: Default::default(),
            title_font: Default::default(),
            heading_font: Default::default(),
            headings: Vec::new(),
            dial_heading: Default::default(),
            wave_fx_heading: Default::default(),
            device_name: Default::default(),
            firmware_label: Default::default(),
            serial_label: Default::default(),
            reveal_button: Default::default(),
            gain_label: Default::default(),
            gain_value: Default::default(),
            gain: Default::default(),
            mute: Default::default(),
            clipguard: Default::default(),
            lowcut: Default::default(),
            output_label: Default::default(),
            output_value: Default::default(),
            output: Default::default(),
            mix_label: Default::default(),
            mix_value: Default::default(),
            mix: Default::default(),
            dial_label: Default::default(),
            dial: Default::default(),
            dsp_button: Default::default(),
            dsp_window: Default::default(),
            dsp: Default::default(),
            status: Default::default(),
            capture_timer: Default::default(),
            test_timer: Default::default(),
            controller: RefCell::new(Some(controller)),
            worker: RefCell::new(None),
            events: EventQueue::new(),
            options,
            model: Cell::new(None),
            serial: RefCell::new(String::new()),
            serial_visible: Cell::new(false),
            syncing: Cell::new(false),
            pending: Cell::new(0),
            test_phase: Cell::new(TestPhase::Connecting),
            test_ticks: Cell::new(0),
            test_mark: Cell::new(0),
            result: RefCell::new(None),
            large_icon: Default::default(),
            small_icon: Default::default(),
        }
    }

    fn shutdown_worker(&self) {
        if let Some(worker) = self.worker.borrow_mut().take() {
            worker.shutdown(&self.events);
        }
    }
}

impl<T, F> NativeUi<NativeAppUi<T, F>> for NativeApp<T, F>
where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    fn build_ui(mut data: NativeApp<T, F>) -> Result<NativeAppUi<T, F>, nwg::NwgError> {
        let resources = nwg::EmbedResource::load(None)?;
        nwg::Icon::builder()
            .source_embed(Some(&resources))
            .source_embed_id(APP_ICON_ID)
            .build(&mut data.large_icon)?;
        nwg::Icon::builder()
            .source_embed(Some(&resources))
            .source_embed_id(APP_ICON_ID)
            .size(Some((16, 16)))
            .build(&mut data.small_icon)?;
        nwg::Window::builder()
            .size((MAIN_WINDOW_WIDTH, MAIN_WINDOW_STATUS_HEIGHT))
            .position((200, 200))
            .title("WaveLite")
            .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
            .icon(Some(&data.small_icon))
            .build(&mut data.window)?;
        set_large_icon(&data.window, &data.large_icon);
        set_system_fonts(&data.window, &mut data.title_font, &mut data.heading_font)?;
        center_window(&data.window);

        data.device_name = label_with_font(
            &data.window,
            "Elgato Wave",
            (11, 11),
            (MAIN_CONTENT_WIDTH, 24),
            &data.title_font,
        )?;
        data.firmware_label = label(&data.window, "Firmware: unavailable", (11, 43), (155, 17))?;
        data.serial_label = label(&data.window, "Serial: ************", (166, 43), (153, 17))?;
        nwg::Button::builder()
            .text("Show")
            .position((334, 38))
            .size((75, 23))
            .enabled(false)
            .parent(&data.window)
            .build(&mut data.reveal_button)?;
        data.headings.push(heading(
            &data.window,
            "Microphone",
            (11, 82),
            &data.heading_font,
        )?);
        data.gain_label = label(&data.window, "Input gain", (11, 111), (240, 17))?;
        data.gain_value = right_label(&data.window, "0.0 dB", (299, 111), (110, 17))?;
        nwg::TrackBar::builder()
            .range(Some(0..400))
            .pos(Some(0))
            .flags(
                nwg::TrackBarFlags::VISIBLE
                    | nwg::TrackBarFlags::HORIZONTAL
                    | nwg::TrackBarFlags::NO_TICK,
            )
            .position((11, 133))
            .size((MAIN_CONTENT_WIDTH, 24))
            .parent(&data.window)
            .build(&mut data.gain)?;
        data.gain.set_enabled(false);
        nwg::CheckBox::builder()
            .text("Mute")
            .position((11, 165))
            .size((MAIN_CONTENT_WIDTH, 17))
            .enabled(false)
            .parent(&data.window)
            .build(&mut data.mute)?;
        nwg::CheckBox::builder()
            .text("Clipguard")
            .position((11, 192))
            .size((MAIN_CONTENT_WIDTH, 17))
            .enabled(false)
            .parent(&data.window)
            .build(&mut data.clipguard)?;
        nwg::CheckBox::builder()
            .text("Enhanced Lowcut")
            .position((11, 219))
            .size((MAIN_CONTENT_WIDTH, 17))
            .enabled(false)
            .parent(&data.window)
            .build(&mut data.lowcut)?;
        data.headings.push(heading(
            &data.window,
            "Output",
            (11, 260),
            &data.heading_font,
        )?);
        data.output_label = label(&data.window, "Headphone volume", (11, 289), (240, 17))?;
        data.output_value = right_label(&data.window, "0.0 dB", (299, 289), (110, 17))?;
        nwg::TrackBar::builder()
            .range(Some(0..1280))
            .pos(Some(0))
            .flags(
                nwg::TrackBarFlags::VISIBLE
                    | nwg::TrackBarFlags::HORIZONTAL
                    | nwg::TrackBarFlags::NO_TICK,
            )
            .position((11, 311))
            .size((MAIN_CONTENT_WIDTH, 24))
            .parent(&data.window)
            .build(&mut data.output)?;
        data.output.set_enabled(false);
        data.mix_label = label(&data.window, "Monitor mix", (11, 352), (240, 17))?;
        data.mix_value = right_label(&data.window, "0%", (299, 352), (110, 17))?;
        nwg::TrackBar::builder()
            .range(Some(0..100))
            .pos(Some(0))
            .flags(
                nwg::TrackBarFlags::VISIBLE
                    | nwg::TrackBarFlags::HORIZONTAL
                    | nwg::TrackBarFlags::NO_TICK,
            )
            .position((11, 374))
            .size((MAIN_CONTENT_WIDTH, 24))
            .parent(&data.window)
            .build(&mut data.mix)?;
        data.mix.set_enabled(false);
        data.dial_heading = heading(&data.window, "Dial", (11, 423), &data.heading_font)?;
        data.wave_fx_heading = heading(&data.window, "Wave FX", (11, 423), &data.heading_font)?;
        data.wave_fx_heading.set_visible(false);
        data.dial_label = label(&data.window, "Function", (11, 452), (80, 23))?;
        nwg::ComboBox::builder()
            .collection(Vec::new())
            .selected_index(Some(0))
            .position((100, 447))
            .size((309, 23))
            .enabled(false)
            .parent(&data.window)
            .build(&mut data.dial)?;
        data.dial.set_visible(false);
        data.dial_label.set_visible(false);
        data.status = label(
            &data.window,
            "Connecting…",
            (11, 480),
            (MAIN_CONTENT_WIDTH, 32),
        )?;
        nwg::Button::builder()
            .text("DSP & EQ…")
            .position((289, 447))
            .size((120, 23))
            .enabled(false)
            .parent(&data.window)
            .build(&mut data.dsp_button)?;
        data.dsp_button.set_visible(false);
        nwg::Window::builder()
            .size((1000, 680))
            .position((240, 120))
            .title("WaveLite — Wave FX")
            .flags(nwg::WindowFlags::WINDOW)
            .icon(Some(&data.small_icon))
            .parent(Some(&data.window))
            .build(&mut data.dsp_window)?;
        set_large_icon(&data.dsp_window, &data.large_icon);
        data.dsp = dsp::DspControls::build(&data.dsp_window)?;

        nwg::AnimationTimer::builder()
            .parent(&data.window)
            .interval(Duration::from_millis(250))
            .active(false)
            .build(&mut data.capture_timer)?;
        nwg::AnimationTimer::builder()
            .parent(&data.window)
            .interval(Duration::from_millis(25))
            .active(data.options.responsiveness_test)
            .build(&mut data.test_timer)?;

        let ui = NativeAppUi {
            inner: Rc::new(data),
            default_handler: RefCell::new(None),
            dsp_handler: RefCell::new(None),
            raw_handler: RefCell::new(None),
        };

        let event_ui = Rc::downgrade(&ui.inner);
        let default_handler =
            nwg::full_bind_event_handler(&ui.window.handle, move |event, _data, handle| {
                let Some(ui) = event_ui.upgrade() else {
                    return;
                };
                use nwg::Event as E;
                match event {
                    E::OnWindowClose if handle == ui.window => ui.close(),
                    E::TrackBarUpdated if handle == ui.gain => ui.change_gain(),
                    E::TrackBarUpdated if handle == ui.output => ui.change_output(),
                    E::TrackBarUpdated if handle == ui.mix => ui.change_mix(),
                    E::OnHorizontalScroll if handle == ui.gain => ui.update_gain_label(),
                    E::OnHorizontalScroll if handle == ui.output => ui.update_output_label(),
                    E::OnHorizontalScroll if handle == ui.mix => ui.update_mix_label(),
                    E::OnButtonClick if handle == ui.mute => ui.change_mute(),
                    E::OnButtonClick if handle == ui.clipguard => ui.change_clipguard(),
                    E::OnButtonClick if handle == ui.lowcut => ui.change_lowcut(),
                    E::OnButtonClick if handle == ui.reveal_button => ui.toggle_serial(),
                    E::OnComboxBoxSelection if handle == ui.dial => ui.change_dial(),
                    E::OnButtonClick if handle == ui.dsp_button => ui.show_dsp(),
                    E::OnTimerTick if handle == ui.capture_timer => ui.capture(),
                    E::OnTimerTick if handle == ui.test_timer => ui.test_tick(),
                    _ => {}
                }
            });
        *ui.default_handler.borrow_mut() = Some(default_handler);

        let dsp_event_ui = Rc::downgrade(&ui.inner);
        let dsp_handler =
            nwg::full_bind_event_handler(&ui.dsp_window.handle, move |event, _data, handle| {
                let Some(ui) = dsp_event_ui.upgrade() else {
                    return;
                };
                use nwg::Event as E;
                match event {
                    E::OnWindowClose if handle == ui.dsp_window => {
                        ui.dsp_window.set_visible(false);
                    }
                    E::TabsContainerChanged if handle == ui.dsp.tabs => ui.fit_dsp_window(),
                    E::TrackBarUpdated => {
                        if let Some(id) = ui.dsp.slider_for(&handle) {
                            ui.change_dsp_slider(id);
                        }
                    }
                    E::OnHorizontalScroll => {
                        if let Some(id) = ui.dsp.slider_for(&handle) {
                            ui.dsp.update(id);
                        }
                    }
                    E::OnButtonClick if handle == ui.dsp.voice_tune => ui.change_voice_tune(),
                    E::OnButtonClick if handle == ui.dsp.expander => ui.change_expander(),
                    E::OnButtonClick if handle == ui.dsp.compressor => ui.change_compressor(),
                    E::OnButtonClick if handle == ui.dsp.eq_enabled => ui.change_eq_enabled(),
                    E::OnButtonClick => {
                        if let Some(index) = ui.dsp.eq_band_for(&handle) {
                            ui.change_eq_band_enabled(index);
                        }
                    }
                    E::OnComboxBoxSelection if handle == ui.dsp.clipguard_mode => {
                        ui.change_clipguard_mode()
                    }
                    E::OnComboxBoxSelection => {
                        if let Some(index) = ui.dsp.eq_kind_for(&handle) {
                            ui.change_eq_kind(index);
                        }
                    }
                    _ => {}
                }
            });
        *ui.dsp_handler.borrow_mut() = Some(dsp_handler);

        let raw_ui = Rc::downgrade(&ui.inner);
        let raw_handler = nwg::bind_raw_event_handler(
            &ui.window.handle,
            RAW_HANDLER_ID,
            move |_hwnd, message, _wparam, _lparam| {
                if message == DEVICE_EVENT_MESSAGE {
                    if let Some(ui) = raw_ui.upgrade() {
                        for event in ui.events.drain() {
                            ui.handle_event(event);
                        }
                    }
                    return Some(0);
                }
                None
            },
        )?;
        *ui.raw_handler.borrow_mut() = Some(raw_handler);
        Ok(ui)
    }
}

impl<T, F> NativeApp<T, F>
where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    fn start(&self) -> std::io::Result<()> {
        let controller = self
            .controller
            .borrow_mut()
            .take()
            .expect("native GUI controller already started");
        let hwnd = HWND(self.window.handle.hwnd().expect("window HWND").cast());
        let worker = Worker::spawn(hwnd, controller, self.events.clone())?;
        *self.worker.borrow_mut() = Some(worker);
        Ok(())
    }

    fn close(&self) {
        self.shutdown_worker();
        nwg::stop_thread_dispatch();
    }

    fn send(&self, command: Command) {
        let is_setting = matches!(command, Command::Set(_));
        if is_setting {
            self.pending.set(self.pending.get().saturating_add(1));
        }
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.send(command);
        } else if is_setting {
            self.pending.set(self.pending.get().saturating_sub(1));
        }
    }

    fn change_gain(&self) {
        if !self.syncing.get() && self.gain.enabled() {
            self.update_gain_label();
            self.send(Command::Set(Setting::Gain(self.gain.pos() as f64 / 10.0)));
        }
    }

    fn set_gain_position(&self, position: usize) {
        set_trackbar_position(&self.gain, position);
    }

    fn update_gain_label(&self) {
        self.gain_value
            .set_text(&format!("{:.1} dB", self.gain.pos() as f64 / 10.0));
    }

    fn change_mute(&self) {
        if !self.syncing.get() && self.mute.enabled() {
            let enabled = self.mute.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::Mute(enabled)));
        }
    }

    fn change_clipguard(&self) {
        if !self.syncing.get() && self.clipguard.enabled() {
            let enabled = self.clipguard.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::Clipguard(enabled)));
        }
    }

    fn change_lowcut(&self) {
        if !self.syncing.get() && self.lowcut.enabled() {
            let enabled = self.lowcut.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::Lowcut(enabled)));
        }
    }

    fn output_offset(&self) -> f64 {
        if self.model.get() == Some(DeviceModel::Wave3Mk2) {
            60.0
        } else {
            128.0
        }
    }

    fn output_value_db(&self) -> f64 {
        self.output.pos() as f64 / 10.0 - self.output_offset()
    }

    fn change_output(&self) {
        if !self.syncing.get() && self.output.enabled() {
            self.update_output_label();
            self.send(Command::Set(Setting::OutputVolume(self.output_value_db())));
        }
    }

    fn set_output_position(&self, db: f64) {
        let position = ((db + self.output_offset()) * 10.0).round().max(0.0) as usize;
        set_trackbar_position(&self.output, position);
        self.update_output_label();
    }

    fn update_output_label(&self) {
        self.output_value
            .set_text(&format!("{:.1} dB", self.output_value_db()));
    }

    fn change_mix(&self) {
        if !self.syncing.get() && self.mix.enabled() {
            self.update_mix_label();
            self.send(Command::Set(Setting::MonitorMix(self.mix.pos() as f64)));
        }
    }

    fn update_mix_label(&self) {
        self.mix_value.set_text(&format!("{}%", self.mix.pos()));
    }

    fn change_dial(&self) {
        if self.syncing.get() || !self.dial.enabled() {
            return;
        }
        let mode = match (self.dial.selection(), self.model.get()) {
            (Some(0), _) => DialMode::InputGain,
            (Some(1), _) => DialMode::OutputVolume,
            (Some(2), Some(DeviceModel::Wave3)) => DialMode::MonitorMix,
            _ => return,
        };
        self.send(Command::Set(Setting::DialMode(mode)));
    }

    fn show_dsp(&self) {
        self.dsp
            .tabs
            .set_selected_tab(self.options.wave_fx_page.unwrap_or(0) as usize);
        self.fit_dsp_window();
        self.dsp_window.set_visible(true);
        self.dsp_window.set_focus();
    }

    fn fit_dsp_window(&self) {
        let eq_selected = self.dsp.tabs.selected_tab() == 1;
        self.dsp
            .tabs
            .set_size(990, if eq_selected { 620 } else { 680 });
        self.dsp_window
            .set_size(1000, if eq_selected { 640 } else { 680 });
        center_window(&self.dsp_window);
    }

    fn set_status_visible(&self, visible: bool) {
        self.status.set_visible(visible);
        self.window.set_size(
            MAIN_WINDOW_WIDTH as u32,
            if visible {
                MAIN_WINDOW_STATUS_HEIGHT
            } else {
                MAIN_WINDOW_HEIGHT
            } as u32,
        );
        center_window(&self.window);
    }

    fn change_clipguard_mode(&self) {
        if self.syncing.get() {
            return;
        }
        let mode = match self.dsp.clipguard_mode.selection() {
            Some(0) => ClipguardMode::Enabled,
            Some(1) => ClipguardMode::Bypassed,
            Some(2) => ClipguardMode::HardClip,
            _ => return,
        };
        self.send(Command::Set(Setting::ClipguardMode(mode)));
    }

    fn change_voice_tune(&self) {
        if !self.syncing.get() && self.dsp.voice_tune.enabled() {
            let enabled = self.dsp.voice_tune.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::VoiceTune(enabled)));
        }
    }

    fn change_expander(&self) {
        if !self.syncing.get() && self.dsp.expander.enabled() {
            let enabled = self.dsp.expander.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::Expander(enabled)));
        }
    }

    fn change_compressor(&self) {
        if !self.syncing.get() && self.dsp.compressor.enabled() {
            let enabled = self.dsp.compressor.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::Compressor(enabled)));
        }
    }

    fn change_eq_enabled(&self) {
        if !self.syncing.get() && self.dsp.eq_enabled.enabled() {
            let enabled = self.dsp.eq_enabled.check_state() == nwg::CheckBoxState::Checked;
            self.send(Command::Set(Setting::EqEnabled(enabled)));
        }
    }

    fn change_eq_band_enabled(&self, index: usize) {
        if self.syncing.get() {
            return;
        }
        self.send(Command::Set(Setting::EqBand {
            index,
            update: EqBandUpdate {
                enabled: Some(self.dsp.eq_band_enabled(index)),
                ..EqBandUpdate::default()
            },
        }));
    }

    fn change_eq_kind(&self, index: usize) {
        if self.syncing.get() {
            return;
        }
        if let Some(kind) = self.dsp.eq_kind(index) {
            self.send(Command::Set(Setting::EqBand {
                index,
                update: EqBandUpdate {
                    kind: Some(kind),
                    ..EqBandUpdate::default()
                },
            }));
        }
    }

    fn change_dsp_slider(&self, id: dsp::DspSliderId) {
        if self.syncing.get() {
            return;
        }
        let value = self.dsp.value(id);
        let setting = match id {
            dsp::DspSliderId::VoiceStrength => Setting::VoiceTuneStrength(value),
            dsp::DspSliderId::ExpanderThreshold => Setting::Dynamics(DynamicsUpdate {
                expander_threshold_db: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::ExpanderRatio => Setting::Dynamics(DynamicsUpdate {
                expander_ratio: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::ExpanderAttack => Setting::Dynamics(DynamicsUpdate {
                expander_attack_ms: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::ExpanderRelease => Setting::Dynamics(DynamicsUpdate {
                expander_release_ms: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::ExpanderMaxReduction => Setting::Dynamics(DynamicsUpdate {
                expander_max_reduction_db: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::CompressorThreshold => Setting::Dynamics(DynamicsUpdate {
                compressor_threshold_db: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::CompressorRatio => Setting::Dynamics(DynamicsUpdate {
                compressor_ratio: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::CompressorAttack => Setting::Dynamics(DynamicsUpdate {
                compressor_attack_ms: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::CompressorRelease => Setting::Dynamics(DynamicsUpdate {
                compressor_release_ms: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::CompressorMakeup => Setting::Dynamics(DynamicsUpdate {
                compressor_makeup_gain_db: Some(value),
                ..DynamicsUpdate::default()
            }),
            dsp::DspSliderId::EqGain(index) => Setting::EqBand {
                index,
                update: EqBandUpdate {
                    gain_db: Some(value),
                    ..EqBandUpdate::default()
                },
            },
            dsp::DspSliderId::EqFrequency(index) => Setting::EqBand {
                index,
                update: EqBandUpdate {
                    frequency_hz: Some(value),
                    ..EqBandUpdate::default()
                },
            },
            dsp::DspSliderId::EqQ(index) => Setting::EqBand {
                index,
                update: EqBandUpdate {
                    q: Some(value),
                    ..EqBandUpdate::default()
                },
            },
        };
        self.send(Command::Set(setting));
    }

    fn toggle_serial(&self) {
        self.serial_visible.set(!self.serial_visible.get());
        self.update_serial();
    }

    fn update_serial(&self) {
        let serial = self.serial.borrow();
        let text = if self.serial_visible.get() && !serial.is_empty() {
            format!("Serial: {serial}")
        } else {
            format!("Serial: {}", "*".repeat(serial.len().max(12)))
        };
        self.serial_label.set_text(&text);
        self.reveal_button.set_text(if self.serial_visible.get() {
            "Hide"
        } else {
            "Show"
        });
    }

    fn render_config(&self, config: &DeviceConfig) {
        if self.pending.get() != 0 {
            return;
        }
        self.syncing.set(true);
        self.set_gain_position((config.gain_db() * 10.0).round().clamp(0.0, 400.0) as usize);
        self.update_gain_label();
        for (control, checked) in [
            (&self.mute, config.muted()),
            (&self.clipguard, config.clipguard()),
            (&self.lowcut, config.lowcut()),
        ] {
            control.set_check_state(if checked {
                nwg::CheckBoxState::Checked
            } else {
                nwg::CheckBoxState::Unchecked
            });
        }
        self.output
            .set_range_max(if self.model.get() == Some(DeviceModel::Wave3Mk2) {
                600
            } else {
                1280
            });
        self.set_output_position(config.output_volume_db());
        set_trackbar_position(
            &self.mix,
            config.monitor_mix_percent().round().clamp(0.0, 100.0) as usize,
        );
        self.update_mix_label();
        if let DeviceConfig::Block16(config) = config {
            let selection = match config.dial_mode() {
                Some(DialMode::InputGain) => Some(0),
                Some(DialMode::OutputVolume) => Some(1),
                Some(DialMode::MonitorMix) if self.model.get() == Some(DeviceModel::Wave3) => {
                    Some(2)
                }
                _ => None,
            };
            self.dial.set_selection(selection);
        }
        if let DeviceConfig::Wave3Mk2(config) = config {
            self.dsp.render(config);
        }
        self.syncing.set(false);
    }

    fn handle_event(&self, event: Event) {
        match event {
            Event::Connected {
                model,
                info,
                config,
            } => {
                self.pending.set(0);
                self.model.set(Some(model));
                self.device_name.set_text(model.display_name());
                self.firmware_label
                    .set_text(&format!("Firmware: {}", info.firmware_version()));
                *self.serial.borrow_mut() = info.serial();
                self.update_serial();
                self.reveal_button.set_enabled(true);
                self.gain.set_enabled(true);
                self.mute.set_enabled(true);
                self.clipguard.set_enabled(true);
                self.lowcut.set_enabled(true);
                self.output.set_enabled(true);
                self.mix.set_enabled(true);
                let dial_entries = if model == DeviceModel::Wave3 {
                    vec![
                        "Input gain".to_owned(),
                        "Output volume".to_owned(),
                        "Mic/PC monitor mix".to_owned(),
                    ]
                } else {
                    vec!["Input gain".to_owned(), "Output volume".to_owned()]
                };
                self.dial.set_collection(dial_entries);
                self.dial_heading.set_visible(!model.is_wave3_mk2());
                self.wave_fx_heading.set_visible(model.is_wave3_mk2());
                self.dial_label.set_text(if model.is_wave3_mk2() {
                    "Processing"
                } else {
                    "Function"
                });
                self.dial_label.set_visible(true);
                self.dial.set_visible(!model.is_wave3_mk2());
                self.dial.set_enabled(!model.is_wave3_mk2());
                self.dsp.set_enabled(model.is_wave3_mk2());
                self.dsp_button.set_visible(model.is_wave3_mk2());
                self.dsp_button.set_enabled(model.is_wave3_mk2());
                if !model.is_wave3_mk2() {
                    self.dsp_window.set_visible(false);
                }
                self.status.set_text("Connected");
                self.set_status_visible(false);
                self.render_config(&config);
                if self.options.wave_fx_page.is_some() && model.is_wave3_mk2() {
                    self.show_dsp();
                }
                if self.options.responsiveness_test {
                    self.test_connected();
                } else if self.options.screenshot.is_some() {
                    self.capture_timer.start();
                }
            }
            Event::ConfigChanged(config) => {
                self.pending.set(self.pending.get().saturating_sub(1));
                self.render_config(&config);
                if self.options.responsiveness_test {
                    self.test_config_changed(&config);
                }
            }
            Event::Refreshed(config) => self.render_config(&config),
            Event::Disconnected(message) => {
                self.pending.set(0);
                self.gain.set_enabled(false);
                self.mute.set_enabled(false);
                self.clipguard.set_enabled(false);
                self.lowcut.set_enabled(false);
                self.output.set_enabled(false);
                self.mix.set_enabled(false);
                self.dial_heading.set_visible(false);
                self.wave_fx_heading.set_visible(false);
                self.dial_label.set_visible(false);
                self.dial.set_visible(false);
                self.dial.set_enabled(false);
                self.dsp.set_enabled(false);
                self.dsp_button.set_enabled(false);
                self.dsp_button.set_visible(false);
                self.dsp_window.set_visible(false);
                self.reveal_button.set_enabled(false);
                self.model.set(None);
                self.status.set_text(&message);
                self.set_status_visible(true);
                if self.options.responsiveness_test {
                    self.finish_test(Err(format!("device disconnected during test: {message}")));
                }
            }
            Event::OperationFailed(message) => {
                self.pending.set(self.pending.get().saturating_sub(1));
                self.status.set_text(&format!("Setting failed: {message}"));
                self.set_status_visible(true);
                if self.options.responsiveness_test {
                    self.finish_test(Err(format!("device operation failed: {message}")));
                }
            }
        }
    }

    fn test_tick(&self) {
        let ticks = self.test_ticks.get() + 1;
        self.test_ticks.set(ticks);
        if self.test_phase.get() != TestPhase::Done && ticks >= 1200 {
            self.finish_test(Err("timed out waiting for delayed device I/O".to_owned()));
        }
    }

    fn test_connected(&self) {
        let ticks = self.test_ticks.get();
        if ticks < 4 {
            self.finish_test(Err(format!(
                "UI was blocked during delayed connection: only {ticks} timer ticks"
            )));
            return;
        }
        self.test_start_step(0);
    }

    fn test_start_step(&self, index: usize) {
        let setting = test_setting(index, self.model.get())
            .expect("test step must exist for the connected model");
        self.test_mark.set(self.test_ticks.get());
        self.test_phase.set(TestPhase::Applying(index));
        self.send(Command::Set(setting));
    }

    fn test_config_changed(&self, config: &DeviceConfig) {
        let TestPhase::Applying(index) = self.test_phase.get() else {
            return;
        };
        if !test_matches(index, config) {
            self.finish_test(Err(format!(
                "unexpected readback after {}",
                test_step_name(index)
            )));
            return;
        }
        let elapsed = self.test_ticks.get().saturating_sub(self.test_mark.get());
        if elapsed < 3 {
            self.finish_test(Err(format!(
                "UI was blocked during {}: only {elapsed} timer ticks",
                test_step_name(index)
            )));
            return;
        }
        let next = index + 1;
        if test_setting(next, self.model.get()).is_some() {
            self.test_start_step(next);
            return;
        }
        println!(
            "Native GUI responsiveness check passed for {} settings: {} UI timer ticks",
            next,
            self.test_ticks.get()
        );
        self.test_phase.set(TestPhase::Done);
        self.test_timer.stop();
        if self.options.screenshot.is_some() {
            self.capture_timer.start();
        } else {
            self.finish_test(Ok(()));
        }
    }

    fn capture(&self) {
        self.capture_timer.stop();
        let result = self
            .options
            .screenshot
            .as_ref()
            .map(|path| {
                let window = if self.dsp_window.visible() {
                    &self.dsp_window
                } else {
                    &self.window
                };
                let hwnd = HWND(window.handle.hwnd().expect("window HWND").cast());
                screenshot::capture_window(hwnd, path)
                    .map_err(|error| error.to_string())
                    .map(|()| println!("Saved screenshot to {}", path.display()))
            })
            .unwrap_or(Ok(()));
        if self.options.responsiveness_test {
            self.finish_test(result);
        } else {
            if let Err(error) = &result {
                eprintln!("Could not save native GUI screenshot: {error}");
                *self.result.borrow_mut() = Some(Err(error.clone()));
            }
            self.close();
        }
    }

    fn finish_test(&self, result: Result<(), String>) {
        if self.result.borrow().is_some() {
            return;
        }
        if let Err(error) = &result {
            eprintln!("Native GUI responsiveness check failed: {error}");
        }
        self.test_phase.set(TestPhase::Done);
        *self.result.borrow_mut() = Some(result);
        self.close();
    }
}

fn test_setting(index: usize, model: Option<DeviceModel>) -> Option<Setting> {
    match index {
        0 => Some(Setting::Gain(13.0)),
        1 => Some(Setting::Mute(true)),
        2 => Some(Setting::Clipguard(false)),
        3 => Some(Setting::Lowcut(false)),
        4 => Some(Setting::OutputVolume(-20.0)),
        5 => Some(Setting::MonitorMix(25.0)),
        6 => match model {
            Some(DeviceModel::Wave3) => Some(Setting::DialMode(DialMode::OutputVolume)),
            Some(DeviceModel::Wave3Mk2) => Some(Setting::ClipguardMode(ClipguardMode::Bypassed)),
            _ => None,
        },
        7 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::VoiceTune(false)),
        8 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::VoiceTuneStrength(65.0)),
        9 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::Expander(true)),
        10 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::Dynamics(DynamicsUpdate {
            expander_threshold_db: Some(-40.0),
            expander_ratio: Some(2.5),
            expander_attack_ms: Some(20.0),
            expander_release_ms: Some(250.0),
            expander_max_reduction_db: Some(30.0),
            ..DynamicsUpdate::default()
        })),
        11 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::Compressor(false)),
        12 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::Dynamics(DynamicsUpdate {
            compressor_threshold_db: Some(-25.0),
            compressor_ratio: Some(6.0),
            compressor_attack_ms: Some(15.0),
            compressor_release_ms: Some(150.0),
            compressor_makeup_gain_db: Some(8.0),
            ..DynamicsUpdate::default()
        })),
        13 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::EqEnabled(true)),
        14 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::EqBand {
            index: 1,
            update: EqBandUpdate {
                enabled: Some(true),
                kind: Some(EqBandType::Notch),
                gain_db: Some(4.5),
                frequency_hz: Some(1200.0),
                q: Some(1.2),
            },
        }),
        15 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::EqBand {
            index: 2,
            update: EqBandUpdate {
                enabled: Some(false),
                kind: Some(EqBandType::HighShelf),
                gain_db: Some(-2.5),
                frequency_hz: Some(2400.0),
                q: Some(0.8),
            },
        }),
        16 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::EqBand {
            index: 3,
            update: EqBandUpdate {
                enabled: Some(false),
                kind: Some(EqBandType::HighPass),
                gain_db: Some(1.5),
                frequency_hz: Some(900.0),
                q: Some(0.7),
            },
        }),
        17 if model == Some(DeviceModel::Wave3Mk2) => Some(Setting::EqBand {
            index: 4,
            update: EqBandUpdate {
                enabled: Some(false),
                kind: Some(EqBandType::LowPass),
                gain_db: Some(-4.0),
                frequency_hz: Some(8000.0),
                q: Some(0.5),
            },
        }),
        _ => None,
    }
}

fn test_matches(index: usize, config: &DeviceConfig) -> bool {
    match index {
        0 => config.gain_db() == 13.0,
        1 => config.muted(),
        2 => !config.clipguard(),
        3 => !config.lowcut(),
        4 => config.output_volume_db() == -20.0,
        5 => config.monitor_mix_percent() == 25.0,
        6 => match config {
            DeviceConfig::Block16(config) => config.dial_mode() == Some(DialMode::OutputVolume),
            DeviceConfig::Wave3Mk2(config) => {
                config.clipguard_mode() == Some(ClipguardMode::Bypassed)
            }
        },
        7 => mk2_matches(config, |config| !config.voice_tune()),
        8 => mk2_matches(config, |config| config.voice_tune_strength_percent() == 65),
        9 => mk2_matches(config, |config| config.expander()),
        10 => mk2_matches(config, |config| {
            let view = config.view();
            let settings = &view.expander_settings;
            settings.threshold_db == -40.0
                && settings.ratio == 2.5
                && settings.attack_ms == 20
                && settings.release_ms == 250
                && settings.max_reduction_db == 30
        }),
        11 => mk2_matches(config, |config| !config.compressor()),
        12 => mk2_matches(config, |config| {
            let view = config.view();
            let settings = &view.compressor_settings;
            settings.threshold_db == -25.0
                && settings.ratio == 6.0
                && settings.attack_ms == 15
                && settings.release_ms == 150
                && settings.makeup_gain_db == 8
        }),
        13 => mk2_matches(config, |config| config.eq_enabled()),
        14 => band_matches(config, 1, true, EqBandType::Notch, 4.5, 1200.0, 1.2),
        15 => band_matches(config, 2, false, EqBandType::HighShelf, -2.5, 2400.0, 0.8),
        16 => band_matches(config, 3, false, EqBandType::HighPass, 1.5, 900.0, 0.7),
        17 => band_matches(config, 4, false, EqBandType::LowPass, -4.0, 8000.0, 0.5),
        _ => false,
    }
}

fn mk2_matches(config: &DeviceConfig, predicate: impl FnOnce(&Wave3Mk2Config) -> bool) -> bool {
    matches!(config, DeviceConfig::Wave3Mk2(config) if predicate(config))
}

fn band_matches(
    config: &DeviceConfig,
    index: usize,
    enabled: bool,
    kind: EqBandType,
    gain: f64,
    frequency: f64,
    q: f64,
) -> bool {
    mk2_matches(config, |config| {
        let view = config.view();
        let band = &view.eq.bands[index - 1];
        let expected_q = 10.0_f64.powf((q.log10() * 64.0).round_ties_even() / 64.0);
        band.enabled == enabled
            && band.kind == kind.as_str()
            && band.gain_db == gain
            && f64::from(band.frequency_hz) == frequency
            && band.q == expected_q
    })
}

fn test_step_name(index: usize) -> &'static str {
    match index {
        0 => "gain write",
        1 => "mute write",
        2 => "Clipguard write",
        3 => "Lowcut write",
        4 => "headphone-volume write",
        5 => "monitor-mix write",
        6 => "dial-function or Clipguard-mode write",
        7 => "voice-tune write",
        8 => "voice-tune-strength write",
        9 => "expander-enable write",
        10 => "expander-settings write",
        11 => "compressor-enable write",
        12 => "compressor-settings write",
        13 => "EQ-enable write",
        14 => "EQ band 1 write",
        15 => "EQ band 2 write",
        16 => "EQ band 3 write",
        17 => "EQ band 4 write",
        _ => "device write",
    }
}

fn set_system_fonts(
    window: &nwg::Window,
    title_font: &mut nwg::Font,
    heading_font: &mut nwg::Font,
) -> Result<(), nwg::NwgError> {
    use windows::Win32::{
        Graphics::Gdi::{CreateFontIndirectW, FW_SEMIBOLD, LOGFONTW},
        UI::{
            HiDpi::{GetDpiForWindow, SystemParametersInfoForDpi},
            WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS},
        },
    };

    fn create_font(log_font: &LOGFONTW) -> Result<nwg::Font, nwg::NwgError> {
        let handle = unsafe { CreateFontIndirectW(log_font) };
        if handle.is_invalid() {
            return Err(nwg::NwgError::resource_create(
                "CreateFontIndirectW failed for the system UI font",
            ));
        }
        Ok(nwg::Font {
            handle: handle.0.cast(),
        })
    }

    let hwnd = HWND(window.handle.hwnd().expect("window HWND").cast());
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            0,
            dpi,
        )
    }
    .map_err(|error| {
        nwg::NwgError::resource_create(format!("SystemParametersInfoForDpi: {error}"))
    })?;
    nwg::Font::set_global_default(Some(create_font(&metrics.lfMessageFont)?));

    let mut title_metrics = metrics.lfMessageFont;
    title_metrics.lfHeight = title_metrics.lfHeight * 4 / 3;
    title_metrics.lfWeight = FW_SEMIBOLD.0 as i32;
    *title_font = create_font(&title_metrics)?;

    let mut heading_metrics = metrics.lfMessageFont;
    heading_metrics.lfWeight = FW_SEMIBOLD.0 as i32;
    *heading_font = create_font(&heading_metrics)?;
    Ok(())
}

fn center_window(window: &nwg::Window) {
    use windows::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        UI::WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        },
    };

    let hwnd = HWND(window.handle.hwnd().expect("window HWND").cast());
    unsafe {
        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return;
        }
        let work = monitor_info.rcWork;
        let x = work.left
            + ((work.right - work.left - (window_rect.right - window_rect.left)) / 2).max(0);
        let y = work.top
            + ((work.bottom - work.top - (window_rect.bottom - window_rect.top)) / 2).max(0);
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

fn set_large_icon(window: &nwg::Window, icon: &nwg::Icon) {
    let hwnd = HWND(window.handle.hwnd().expect("window HWND").cast());
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WM_SETICON,
            Some(WPARAM(
                windows::Win32::UI::WindowsAndMessaging::ICON_BIG as usize,
            )),
            Some(LPARAM(icon.handle as isize)),
        )
    };
}

fn heading(
    parent: &nwg::Window,
    text: &str,
    position: (i32, i32),
    font: &nwg::Font,
) -> Result<nwg::Label, nwg::NwgError> {
    label_with_font(parent, text, position, (MAIN_CONTENT_WIDTH, 17), font)
}

fn right_label(
    parent: &nwg::Window,
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
) -> Result<nwg::Label, nwg::NwgError> {
    let mut label = nwg::Label::default();
    nwg::Label::builder()
        .text(text)
        .position(position)
        .size(size)
        .h_align(nwg::HTextAlign::Right)
        .parent(parent)
        .build(&mut label)?;
    Ok(label)
}

fn label_with_font(
    parent: &nwg::Window,
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
    font: &nwg::Font,
) -> Result<nwg::Label, nwg::NwgError> {
    let mut label = nwg::Label::default();
    nwg::Label::builder()
        .text(text)
        .position(position)
        .size(size)
        .font(Some(font))
        .parent(parent)
        .build(&mut label)?;
    Ok(label)
}

fn label(
    parent: &nwg::Window,
    text: &str,
    position: (i32, i32),
    size: (i32, i32),
) -> Result<nwg::Label, nwg::NwgError> {
    let mut label = nwg::Label::default();
    nwg::Label::builder()
        .text(text)
        .position(position)
        .size(size)
        .parent(parent)
        .build(&mut label)?;
    Ok(label)
}

fn set_trackbar_position(trackbar: &nwg::TrackBar, position: usize) {
    let hwnd = HWND(trackbar.handle.hwnd().expect("trackbar HWND").cast());
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            windows::Win32::UI::Controls::TBM_SETPOS,
            Some(WPARAM(1)),
            Some(LPARAM(position as isize)),
        )
    };
}

pub fn run<T, F>(controller: Controller<T, F>, options: Options) -> Result<(), Box<dyn Error>>
where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    nwg::init()?;
    let app = NativeApp::build_ui(NativeApp::new(controller, options))?;
    app.start()?;
    nwg::dispatch_thread_events();
    if let Some(result) = app.result.borrow_mut().take() {
        result.map_err(|error| std::io::Error::other(error))?;
    }
    Ok(())
}
