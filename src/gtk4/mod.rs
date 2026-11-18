mod dsp;
mod udev;
mod worker;

use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use crate::{
    controller::{Command, Controller, DeviceConnection, Event, Setting},
    device::{DeviceConfig, DeviceModel, Transport},
    protocol::{DeviceInfo, DialMode},
};
use async_channel::Sender;
use gtk::{
    Align, Application, ApplicationWindow, Box, Button, DropDown, Frame, HeaderBar, Image, Label,
    Orientation, Scale, ScrolledWindow, Separator, StringList, Switch, TextView, ToggleButton,
    glib::{self, ControlFlow, SourceId},
    prelude::*,
};

const APP_ID: &str = "dev.wavelite.WaveLite";
const DEBOUNCE: Duration = Duration::from_millis(200);
const SERIAL_LENGTH: usize = 12;
const SERIAL_LABEL_WIDTH: i32 = 20;
const INSTALL_RULE_LABEL: &str = "Install USB Access Rule…";

#[derive(Clone, Copy)]
enum StatusLevel {
    Warning,
    Error,
}

#[derive(Clone)]
struct CommandSender {
    sender: Sender<Command>,
    pending: Rc<Cell<usize>>,
}

impl CommandSender {
    fn new(sender: Sender<Command>) -> Self {
        Self {
            sender,
            pending: Rc::new(Cell::new(0)),
        }
    }

    fn send(&self, command: Command) {
        let is_setting = matches!(command, Command::Set(_));
        if is_setting {
            self.pending.set(self.pending.get().saturating_add(1));
        }
        if self.sender.try_send(command).is_err() && is_setting {
            self.complete();
        }
    }

    fn shutdown(&self) {
        let _ = self.sender.try_send(Command::Shutdown);
        self.sender.close();
    }

    fn complete(&self) {
        self.pending.set(self.pending.get().saturating_sub(1));
    }

    fn reset(&self) {
        self.pending.set(0);
    }

    fn has_pending(&self) -> bool {
        self.pending.get() != 0
    }
}

struct Slider {
    row: Box,
    scale: Scale,
    value_label: Label,
    digits: usize,
    unit: &'static str,
}

impl Slider {
    fn render(&self, value: f64) {
        self.scale.set_value(value);
        update_value_label(&self.value_label, value, self.digits, self.unit);
    }
}

struct Ui {
    window: ApplicationWindow,
    commands: CommandSender,
    controls: Box,
    connected: Cell<bool>,
    manage_udev: bool,
    udev_group: Box,
    udev_button: Button,
    udev_installed: Cell<bool>,
    udev_installing: Cell<bool>,
    device_name: Label,
    model: Cell<DeviceModel>,
    serial_label: Label,
    serial_toggle: ToggleButton,
    firmware_label: Label,
    serial: Rc<RefCell<String>>,
    serial_visible: Rc<Cell<bool>>,
    status: Box,
    status_label: Label,
    gain: Slider,
    mute: Switch,
    clipguard: Switch,
    lowcut: Switch,
    output_volume: Slider,
    monitor_mix: Slider,
    dial: DropDown,
    dial_group: Box,
    dsp: Rc<dsp::DspUi>,
    dsp_group: Box,
    show_dsp_on_connect: bool,
    syncing: Rc<Cell<bool>>,
    gain_pending: Rc<RefCell<Option<SourceId>>>,
    output_pending: Rc<RefCell<Option<SourceId>>>,
    mix_pending: Rc<RefCell<Option<SourceId>>>,
}

impl Ui {
    fn update_control_access(&self) {
        let sensitive = controls_sensitive(self.udev_installed.get(), self.connected.get());
        self.controls.set_sensitive(sensitive);
        self.serial_toggle
            .set_sensitive(sensitive && !self.serial.borrow().is_empty());
    }

    fn set_udev_installed(&self, installed: bool) {
        let changed = self.udev_installed.get() != installed;
        self.udev_installed.set(installed);
        self.udev_group.set_visible(!installed);
        self.udev_button
            .set_sensitive(!installed && !self.udev_installing.get());
        self.update_control_access();

        if !installed {
            self.dsp.hide();
            self.status.set_visible(false);
        } else if changed && self.connected.get() {
            self.status.set_visible(false);
        } else if changed {
            self.show_status(
                "USB access rule installed. Reconnect the microphone if it does not appear.",
                StatusLevel::Warning,
            );
        }
        self.fit_window_to_content();
    }

    fn refresh_udev(&self) {
        if self.manage_udev && !self.udev_installing.get() {
            let installed = udev::is_installed();
            if installed != self.udev_installed.get() {
                self.set_udev_installed(installed);
            }
        }
    }

    fn confirm_udev_install(self: &Rc<Self>) {
        if self.udev_installing.replace(true) {
            return;
        }
        self.udev_button.set_sensitive(false);

        let dialog = ApplicationWindow::builder()
            .transient_for(&self.window)
            .modal(true)
            .destroy_with_parent(true)
            .title("Install USB Access Rule")
            .default_width(640)
            .default_height(420)
            .build();
        dialog.set_titlebar(Some(&HeaderBar::new()));
        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        dialog.set_child(Some(&content));

        let explanation = Label::new(Some(
            "WaveLite will ask for administrator authorization with pkexec and install this udev rule:",
        ));
        explanation.set_xalign(0.0);
        explanation.set_wrap(true);
        content.append(&explanation);

        let location = Label::new(Some(&format!("Install location: {}", udev::RULE_PATH)));
        location.set_xalign(0.0);
        location.set_selectable(true);
        content.append(&location);

        let rule_view = TextView::new();
        rule_view.set_editable(false);
        rule_view.set_cursor_visible(false);
        rule_view.set_monospace(true);
        rule_view.set_wrap_mode(gtk::WrapMode::WordChar);
        rule_view.buffer().set_text(udev::RULE);
        rule_view.update_property(&[gtk::accessible::Property::Label("udev rule content")]);
        let scroll = ScrolledWindow::new();
        scroll.set_min_content_width(560);
        scroll.set_min_content_height(180);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&rule_view));
        content.append(&scroll);

        let actions = Box::new(Orientation::Horizontal, 12);
        actions.set_halign(Align::End);
        content.append(&actions);
        let cancel = Button::with_label("Cancel");
        actions.append(&cancel);
        let install = Button::with_label("Install…");
        install.add_css_class("suggested-action");
        actions.append(&install);
        dialog.set_default_widget(Some(&cancel));

        let accepted = Rc::new(Cell::new(false));
        cancel.connect_clicked({
            let dialog = dialog.clone();
            move |_| dialog.close()
        });
        install.connect_clicked({
            let dialog = dialog.clone();
            let accepted = accepted.clone();
            let ui = self.clone();
            move |_| {
                accepted.set(true);
                dialog.close();
                ui.begin_udev_install();
            }
        });
        dialog.connect_close_request({
            let accepted = accepted.clone();
            let ui = Rc::downgrade(self);
            move |_| {
                if !accepted.get() {
                    if let Some(ui) = ui.upgrade() {
                        ui.udev_installing.set(false);
                        ui.udev_button.set_sensitive(true);
                    }
                }
                glib::Propagation::Proceed
            }
        });
        dialog.present();
    }

    fn begin_udev_install(self: &Rc<Self>) {
        self.udev_button.set_label("Installing…");
        self.show_status(
            "Waiting for administrator authorization to install the USB access rule…",
            StatusLevel::Warning,
        );
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(udev::install).await;
            ui.udev_installing.set(false);
            ui.udev_button.set_label(INSTALL_RULE_LABEL);
            match result {
                Ok(Ok(())) => ui.set_udev_installed(true),
                Ok(Err(error)) => {
                    ui.udev_button.set_sensitive(true);
                    ui.show_status(error, StatusLevel::Error);
                }
                Err(_) => {
                    ui.udev_button.set_sensitive(true);
                    ui.show_status(
                        "The udev installer stopped unexpectedly. Try again or use the manual setup instructions.",
                        StatusLevel::Error,
                    );
                }
            }
        });
    }

    fn render_config(&self, config: &DeviceConfig) {
        if self.commands.has_pending() {
            return;
        }
        self.syncing.set(true);
        if self.gain_pending.borrow().is_none() {
            self.gain.render(config.gain_db());
        }
        self.mute.set_active(config.muted());
        self.clipguard.set_active(config.clipguard());
        self.lowcut.set_active(config.lowcut());
        if self.output_pending.borrow().is_none() {
            self.output_volume.render(config.output_volume_db());
        }
        if self.mix_pending.borrow().is_none() {
            self.monitor_mix.render(config.monitor_mix_percent());
        }
        if let DeviceConfig::Block16(config) = config {
            let dial_mode = config.dial_mode();
            match dial_mode {
                Some(DialMode::InputGain) => self.dial.set_selected(0),
                Some(DialMode::OutputVolume) => self.dial.set_selected(1),
                Some(DialMode::MonitorMix) if self.model.get() == DeviceModel::Wave3 => {
                    self.dial.set_selected(2)
                }
                _ => {}
            }
            let known = match dial_mode {
                Some(DialMode::MonitorMix) => self.model.get() == DeviceModel::Wave3,
                Some(_) => true,
                None => false,
            };
            self.dial
                .set_sensitive(self.controls.is_sensitive() && known);
            if !known {
                self.show_status(
                    format!("Unknown dial function: {}", config.raw()[12]),
                    StatusLevel::Warning,
                );
            }
        }
        self.syncing.set(false);
        if let DeviceConfig::Wave3Mk2(config) = config {
            self.dsp.render_config(config);
        }
    }

    fn fit_window_to_content(&self) {
        self.window.set_default_size(self.window.width(), 1);
    }

    fn render_connected(&self, model: DeviceModel, info: DeviceInfo, config: DeviceConfig) {
        self.commands.reset();
        self.connected.set(true);
        self.model.set(model);
        self.device_name.set_markup(&format!(
            "<span size='x-large' weight='bold'>{}</span>",
            model.display_name()
        ));
        self.output_volume
            .scale
            .set_range(if model.is_wave3_mk2() { -60.0 } else { -128.0 }, 0.0);
        let dial_entries: &[&str] = if model == DeviceModel::Wave3 {
            &["Input gain", "Output volume", "Mic/PC monitor mix"]
        } else {
            &["Input gain", "Output volume"]
        };
        self.dial.set_model(Some(&StringList::new(dial_entries)));
        self.dial_group.set_visible(!model.is_wave3_mk2());
        self.dsp_group.set_visible(model.is_wave3_mk2());
        if !model.is_wave3_mk2() {
            self.dsp.hide();
        } else if self.show_dsp_on_connect {
            self.dsp.present();
        }
        *self.serial.borrow_mut() = info.serial();
        self.update_control_access();
        self.firmware_label
            .set_text(&format!("Firmware: {}", info.firmware_version()));
        self.update_serial();
        self.status.set_visible(false);
        self.render_config(&config);
        self.fit_window_to_content();
    }

    fn render_disconnected(&self, message: &str) {
        self.commands.reset();
        self.connected.set(false);
        self.cancel_pending();
        self.dsp.hide();
        self.dsp_group.set_visible(false);
        self.controls.set_sensitive(false);
        self.udev_group.set_visible(!self.udev_installed.get());
        self.serial.borrow_mut().clear();
        self.serial_visible.set(false);
        self.serial_toggle.set_active(false);
        self.serial_toggle.set_sensitive(false);
        self.update_serial();
        self.firmware_label.set_text("Firmware: unavailable");
        if !self.udev_installed.get() {
            self.status.set_visible(false);
            self.fit_window_to_content();
            return;
        }
        let message =
            if message.contains("Access denied") || message.contains("insufficient permissions") {
                format!("{message}\nCheck the USB permissions and reconnect the microphone.")
            } else {
                message.to_owned()
            };
        self.show_status(message, StatusLevel::Error);
    }

    fn update_serial(&self) {
        self.serial_label.set_text(&serial_text(
            &self.serial.borrow(),
            self.serial_visible.get(),
        ));
        update_serial_toggle(&self.serial_toggle, self.serial_visible.get());
    }

    fn show_status(&self, message: impl AsRef<str>, level: StatusLevel) {
        self.status_label.set_text(message.as_ref());
        self.status_label.remove_css_class("error");
        self.status_label.remove_css_class("warning");
        self.status_label.add_css_class(match level {
            StatusLevel::Warning => "warning",
            StatusLevel::Error => "error",
        });
        self.status.set_visible(true);
        self.fit_window_to_content();
    }

    fn cancel_pending(&self) {
        for pending in [&self.gain_pending, &self.output_pending, &self.mix_pending] {
            if let Some(source) = pending.borrow_mut().take() {
                source.remove();
            }
        }
    }
}

pub fn run<T, F>(controller: Controller<T, F>, initial_dsp_page: Option<u32>)
where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    run_inner(controller, initial_dsp_page, None, true);
}

pub fn run_with_screenshot<T, F>(
    controller: Controller<T, F>,
    initial_dsp_page: Option<u32>,
    screenshot: PathBuf,
) where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    run_inner(controller, initial_dsp_page, Some(screenshot), true);
}

pub fn run_demo<T, F>(controller: Controller<T, F>, initial_dsp_page: Option<u32>)
where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    run_inner(controller, initial_dsp_page, None, false);
}

pub fn run_demo_with_screenshot<T, F>(
    controller: Controller<T, F>,
    initial_dsp_page: Option<u32>,
    screenshot: PathBuf,
) where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    run_inner(controller, initial_dsp_page, Some(screenshot), false);
}

fn run_inner<T, F>(
    controller: Controller<T, F>,
    initial_dsp_page: Option<u32>,
    screenshot: Option<PathBuf>,
    manage_udev: bool,
) where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    let controller = Rc::new(RefCell::new(Some(controller)));
    let application = Application::builder().application_id(APP_ID).build();
    application.connect_activate(move |application| {
        let Some(controller) = controller.borrow_mut().take() else {
            if let Some(window) = application.active_window() {
                window.present();
            }
            return;
        };
        build_window(
            application,
            controller,
            initial_dsp_page,
            screenshot.clone(),
            manage_udev,
        );
    });
    application.run();
}

fn build_window<T, F>(
    application: &Application,
    controller: Controller<T, F>,
    initial_dsp_page: Option<u32>,
    screenshot: Option<PathBuf>,
    manage_udev: bool,
) where
    T: Transport + Send + 'static,
    F: FnMut() -> DeviceConnection<T> + Send + 'static,
{
    gtk::Window::set_default_icon_name(APP_ID);
    install_styles();
    let (sender, command_receiver) = async_channel::unbounded();
    let commands = CommandSender::new(sender);
    let window = ApplicationWindow::builder()
        .application(application)
        .title("WaveLite")
        .width_request(480)
        .default_width(480)
        .build();
    window.set_titlebar(Some(&HeaderBar::new()));

    let root = Box::new(Orientation::Vertical, 0);
    window.set_child(Some(&root));

    let status = Box::new(Orientation::Vertical, 0);
    status.set_margin_top(12);
    status.set_margin_bottom(12);
    status.set_margin_start(12);
    status.set_margin_end(12);
    let status_label = Label::new(Some("Connecting…"));
    status_label.set_xalign(0.0);
    status_label.set_wrap(true);
    status.append(&status_label);
    status.set_visible(true);

    let content = Box::new(Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    root.append(&content);
    root.append(&status);

    let device = Box::new(Orientation::Vertical, 6);
    content.append(&device);
    let device_name = Label::new(Some("Elgato Wave:1"));
    device_name.set_markup("<span size='x-large' weight='bold'>Elgato Wave:1</span>");
    device_name.set_xalign(0.0);
    device.append(&device_name);

    let serial_row = Box::new(Orientation::Horizontal, 12);
    device.append(&serial_row);
    let firmware_label = Label::new(Some("Firmware: unavailable"));
    firmware_label.set_xalign(0.0);
    firmware_label.add_css_class("dim-label");
    serial_row.append(&firmware_label);

    let serial_label = Label::new(Some(&serial_text("", false)));
    serial_label.set_width_chars(SERIAL_LABEL_WIDTH);
    serial_label.set_xalign(0.0);
    serial_label.add_css_class("dim-label");
    serial_row.append(&serial_label);
    let serial_toggle = ToggleButton::new();
    serial_toggle.add_css_class("flat");
    serial_toggle.add_css_class("dim-label");
    update_serial_toggle(&serial_toggle, false);
    serial_toggle.set_sensitive(false);
    serial_row.append(&serial_toggle);

    let udev_group = Box::new(Orientation::Vertical, 6);
    let udev_label = Label::new(Some(
        "WaveLite's USB access rule is missing or outdated. Install it to enable microphone controls.",
    ));
    udev_label.set_xalign(0.0);
    udev_label.set_wrap(true);
    udev_group.append(&udev_label);
    let udev_button = Button::with_label(INSTALL_RULE_LABEL);
    udev_button.set_halign(Align::Start);
    udev_group.append(&udev_button);
    let udev_installed = !manage_udev || udev::is_installed();
    udev_group.set_visible(!udev_installed);
    udev_button.set_sensitive(!udev_installed);
    content.append(&udev_group);

    let controls = Box::new(Orientation::Vertical, 12);
    controls.set_sensitive(false);
    content.append(&controls);

    let gain = slider_row("Input gain", 0.0, 40.0, 0.1, 1, "dB");
    let (mute_row, mute) = switch_row("Mute");
    let (clipguard_row, clipguard) = switch_row("Clipguard");
    let (lowcut_row, lowcut) = switch_row("Enhanced Lowcut");
    let (microphone, microphone_rows) = control_group("Microphone");
    append_controls(
        &microphone_rows,
        &[&gain.row, &mute_row, &clipguard_row, &lowcut_row],
    );
    controls.append(&microphone);

    let output_volume = slider_row("Headphone volume", -128.0, 0.0, 0.1, 1, "dB");
    let monitor_mix = slider_row("Monitor mix", 0.0, 100.0, 1.0, 0, "%");
    let (output, output_rows) = control_group("Output");
    append_controls(&output_rows, &[&output_volume.row, &monitor_mix.row]);
    controls.append(&output);

    let dial_model = StringList::new(&["Input gain", "Output volume"]);
    let dial = DropDown::new(Some(dial_model), None::<&gtk::Expression>);
    let dial_row = value_row("Function", &dial);
    let (dial_group, dial_rows) = control_group("Dial");
    dial_rows.append(&dial_row);
    controls.append(&dial_group);

    let dsp_button = Button::with_label("DSP & EQ…");
    let (dsp_group, dsp_rows) = control_group("Wave FX");
    dsp_rows.append(&value_row("Processing", &dsp_button));
    controls.append(&dsp_group);
    dsp_group.set_visible(false);

    let sender = commands.clone();
    let syncing = Rc::new(Cell::new(false));
    let gain_pending = Rc::new(RefCell::new(None));
    let output_pending = Rc::new(RefCell::new(None));
    let mix_pending = Rc::new(RefCell::new(None));
    let serial = Rc::new(RefCell::new(String::new()));
    let serial_visible = Rc::new(Cell::new(false));

    bind_slider(
        &gain,
        &sender,
        syncing.clone(),
        gain_pending.clone(),
        Setting::Gain,
    );
    bind_slider(
        &output_volume,
        &sender,
        syncing.clone(),
        output_pending.clone(),
        Setting::OutputVolume,
    );
    bind_slider(
        &monitor_mix,
        &sender,
        syncing.clone(),
        mix_pending.clone(),
        Setting::MonitorMix,
    );
    bind_switch(&mute, &sender, syncing.clone(), Setting::Mute);
    bind_switch(&clipguard, &sender, syncing.clone(), Setting::Clipguard);
    bind_switch(&lowcut, &sender, syncing.clone(), Setting::Lowcut);
    dial.connect_selected_notify({
        let sender = sender.clone();
        let syncing = syncing.clone();
        move |dial| {
            if syncing.get() || !dial.is_sensitive() {
                return;
            }
            let mode = match dial.selected() {
                0 => DialMode::InputGain,
                1 => DialMode::OutputVolume,
                2 => DialMode::MonitorMix,
                _ => return,
            };
            sender.send(Command::Set(Setting::DialMode(mode)));
        }
    });
    serial_toggle.connect_toggled({
        let serial = serial.clone();
        let serial_visible = serial_visible.clone();
        let serial_label = serial_label.clone();
        move |toggle| {
            let visible = toggle.is_active();
            serial_visible.set(visible);
            serial_label.set_text(&serial_text(&serial.borrow(), visible));
            update_serial_toggle(toggle, visible);
        }
    });

    let dsp = dsp::DspUi::new(&window, sender.clone(), initial_dsp_page.unwrap_or(0));
    dsp_button.connect_clicked({
        let dsp = dsp.clone();
        move |_| dsp.present()
    });

    let ui = Rc::new(Ui {
        window: window.clone(),
        commands: sender.clone(),
        controls,
        connected: Cell::new(false),
        manage_udev,
        udev_group,
        udev_button,
        udev_installed: Cell::new(udev_installed),
        udev_installing: Cell::new(false),
        device_name,
        model: Cell::new(DeviceModel::Wave1),
        serial_label,
        serial_toggle,
        firmware_label,
        serial,
        serial_visible,
        status,
        status_label,
        gain,
        mute,
        clipguard,
        lowcut,
        output_volume,
        monitor_mix,
        dial,
        dial_group,
        dsp,
        dsp_group,
        show_dsp_on_connect: initial_dsp_page.is_some(),
        syncing,
        gain_pending,
        output_pending,
        mix_pending,
    });

    ui.udev_button.connect_clicked({
        let ui = Rc::downgrade(&ui);
        move |_| {
            if let Some(ui) = ui.upgrade() {
                ui.confirm_udev_install();
            }
        }
    });
    if manage_udev {
        glib::timeout_add_local(Duration::from_secs(2), {
            let ui = Rc::downgrade(&ui);
            move || {
                let Some(ui) = ui.upgrade() else {
                    return ControlFlow::Break;
                };
                ui.refresh_udev();
                ControlFlow::Continue
            }
        });
        if !udev_installed {
            ui.status.set_visible(false);
        }
    }

    let screenshot_window = window.clone();
    let on_event = move |event| match event {
        Event::Connected {
            model,
            info,
            config,
        } => {
            ui.render_connected(model, info, config);
            if let Some(path) = screenshot.clone() {
                schedule_screenshot(&screenshot_window, &ui.dsp, path);
            }
        }
        Event::ConfigChanged(config) => {
            ui.commands.complete();
            ui.render_config(&config);
        }
        Event::Refreshed(config) => ui.render_config(&config),
        Event::Disconnected(message) => ui.render_disconnected(&message),
        Event::OperationFailed(message) => {
            ui.commands.complete();
            ui.show_status(format!("Setting failed: {message}"), StatusLevel::Error);
        }
    };
    window.present();
    glib::idle_add_local_once(move || worker::spawn_with(command_receiver, controller, on_event));

    application.connect_shutdown(move |_| sender.shutdown());
}

fn schedule_screenshot(window: &ApplicationWindow, dsp: &Rc<dsp::DspUi>, path: PathBuf) {
    let window = window.clone();
    let dsp = dsp.clone();
    glib::timeout_add_local_once(Duration::from_millis(250), move || {
        let application = window.application().expect("window application");
        let target = dsp
            .visible_window()
            .or_else(|| application.active_window())
            .unwrap_or_else(|| window.clone().into());
        match save_screenshot(&target, &path) {
            Ok(()) => println!("Saved screenshot to {}", path.display()),
            Err(error) => eprintln!("Could not save screenshot: {error}"),
        }
        application.quit();
    });
}

fn save_screenshot(
    window: &gtk::Window,
    path: &Path,
) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let width = window.width();
    let height = window.height();
    if width <= 0 || height <= 0 {
        return Err(std::io::Error::other("window has not been allocated").into());
    }

    let snapshot = gtk::Snapshot::new();
    let paintable = gtk::WidgetPaintable::new(Some(window));
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot
        .to_node()
        .ok_or_else(|| std::io::Error::other("window rendered no nodes"))?;
    let surface = window
        .surface()
        .ok_or_else(|| std::io::Error::other("window has no drawing surface"))?;
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer.realize(Some(&surface))?;
    let texture = renderer.render_texture(node, None);
    let result = texture.save_to_png(path);
    renderer.unrealize();
    result?;
    Ok(())
}

fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".setting-list { background-color: @theme_base_color; border-color: @borders; }\n\
         .setting-list separator { background-color: @borders; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn control_group(title: &str) -> (Box, Box) {
    let group = Box::new(Orientation::Vertical, 6);
    let label = Label::new(None);
    label.set_markup(&format!("<b>{title}</b>"));
    label.set_xalign(0.0);
    group.append(&label);
    let rows = Box::new(Orientation::Vertical, 0);
    let frame = Frame::new(None);
    frame.add_css_class("setting-list");
    frame.set_child(Some(&rows));
    group.append(&frame);
    (group, rows)
}

fn append_controls(rows: &Box, controls: &[&Box]) {
    for (index, row) in controls.iter().copied().enumerate() {
        if index > 0 {
            rows.append(&Separator::new(Orientation::Horizontal));
        }
        rows.append(row);
    }
}

fn list_row(content: &Box) -> Box {
    content.clone()
}

fn value_row(title: &str, control: &impl IsA<gtk::Widget>) -> Box {
    let content = Box::new(Orientation::Horizontal, 12);
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let label = Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    content.append(&label);
    content.append(control);
    list_row(&content)
}

fn switch_row(title: &str) -> (Box, Switch) {
    let switch = Switch::new();
    switch.set_valign(Align::Center);
    (value_row(title, &switch), switch)
}

fn slider_row(
    title: &str,
    minimum: f64,
    maximum: f64,
    step: f64,
    digits: usize,
    unit: &'static str,
) -> Slider {
    let content = Box::new(Orientation::Vertical, 6);
    content.set_margin_top(12);
    content.set_margin_bottom(6);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let heading = Box::new(Orientation::Horizontal, 12);
    content.append(&heading);
    let label = Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    heading.append(&label);
    let value_label = Label::new(None);
    value_label.set_halign(Align::End);
    heading.append(&value_label);

    let scale = Scale::with_range(Orientation::Horizontal, minimum, maximum, step);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.update_property(&[gtk::accessible::Property::Label(title)]);
    content.append(&scale);

    let slider = Slider {
        row: list_row(&content),
        scale,
        value_label,
        digits,
        unit,
    };
    slider.render(minimum);
    slider
}

fn update_value_label(label: &Label, value: f64, digits: usize, unit: &str) {
    let separator = if unit == "%" { "" } else { " " };
    label.set_text(&format!("{value:.digits$}{separator}{unit}"));
}

fn update_serial_toggle(toggle: &ToggleButton, visible: bool) {
    let (icon, label) = if visible {
        ("view-conceal-symbolic", "Hide serial number")
    } else {
        ("view-reveal-symbolic", "Show serial number")
    };
    toggle.set_child(Some(&Image::from_icon_name(icon)));
    toggle.set_tooltip_text(Some(label));
    toggle.update_property(&[gtk::accessible::Property::Label(label)]);
}

fn serial_text(serial: &str, visible: bool) -> String {
    if visible && !serial.is_empty() {
        format!("Serial: {serial}")
    } else {
        let length = if serial.is_empty() {
            SERIAL_LENGTH
        } else {
            serial.len()
        };
        format!("Serial: {}", "*".repeat(length))
    }
}

fn controls_sensitive(udev_installed: bool, connected: bool) -> bool {
    udev_installed && connected
}

fn bind_slider(
    slider: &Slider,
    sender: &CommandSender,
    syncing: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<SourceId>>>,
    setting: impl Fn(f64) -> Setting + Clone + 'static,
) {
    let sender = sender.clone();
    let value_label = slider.value_label.clone();
    let digits = slider.digits;
    let unit = slider.unit;
    slider.scale.connect_value_changed(move |scale| {
        let value = scale.value();
        update_value_label(&value_label, value, digits, unit);
        if syncing.get() || !scale.is_sensitive() {
            return;
        }
        if let Some(source) = pending.borrow_mut().take() {
            source.remove();
        }
        let sender = sender.clone();
        let timer_pending = pending.clone();
        let setting = setting.clone();
        let source = glib::timeout_add_local_once(DEBOUNCE, move || {
            *timer_pending.borrow_mut() = None;
            sender.send(Command::Set(setting(value)));
        });
        *pending.borrow_mut() = Some(source);
    });
}

fn bind_switch(
    switch: &Switch,
    sender: &CommandSender,
    syncing: Rc<Cell<bool>>,
    setting: impl Fn(bool) -> Setting + 'static,
) {
    let sender = sender.clone();
    switch.connect_active_notify(move |switch| {
        if !syncing.get() && switch.is_sensitive() {
            sender.send(Command::Set(setting(switch.is_active())));
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::{controller::Command, device::Setting};

    use super::{CommandSender, controls_sensitive, serial_text};

    #[test]
    fn controls_require_usb_access_and_a_connection() {
        assert!(!controls_sensitive(false, false));
        assert!(!controls_sensitive(false, true));
        assert!(!controls_sensitive(true, false));
        assert!(controls_sensitive(true, true));
    }

    #[test]
    fn pending_writes_are_tracked_in_order() {
        let (sender, _receiver) = async_channel::unbounded();
        let commands = CommandSender::new(sender);
        let cloned = commands.clone();
        cloned.send(Command::Set(Setting::Gain(12.5)));
        cloned.send(Command::Set(Setting::Mute(true)));
        assert!(commands.has_pending());
        commands.complete();
        assert!(commands.has_pending());
        commands.complete();
        assert!(!commands.has_pending());
    }

    #[test]
    fn serial_requires_explicit_reveal() {
        let hidden = serial_text("N8Q3V7K2X9L4", false);
        let visible = serial_text("N8Q3V7K2X9L4", true);
        assert_eq!(hidden, "Serial: ************");
        assert_eq!(visible, "Serial: N8Q3V7K2X9L4");
        assert_eq!(hidden.len(), visible.len());
        assert_eq!(serial_text("", true), "Serial: ************");
    }
}
