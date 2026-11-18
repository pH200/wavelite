use crate::wave3_mk2::{ClipguardMode, DynamicsUpdate, EqBandType, EqBandUpdate, Wave3Mk2Config};
use gtk::{Grid, Stack, StackSwitcher};

use super::*;

const EQ_BAND_TYPES: [EqBandType; 6] = [
    EqBandType::HighPass,
    EqBandType::LowShelf,
    EqBandType::Bell,
    EqBandType::Notch,
    EqBandType::HighShelf,
    EqBandType::LowPass,
];
const EQ_BAND_TYPE_LABELS: [&str; 6] = [
    "High pass",
    "Low shelf",
    "Bell",
    "Notch",
    "High shelf",
    "Low pass",
];

struct DspSlider {
    slider: Slider,
    pending: Rc<RefCell<Option<SourceId>>>,
}

impl DspSlider {
    fn new(
        title: &str,
        minimum: f64,
        maximum: f64,
        step: f64,
        digits: usize,
        unit: &'static str,
    ) -> Self {
        Self {
            slider: slider_row(title, minimum, maximum, step, digits, unit),
            pending: Rc::new(RefCell::new(None)),
        }
    }

    fn render(&self, value: f64) {
        if self.pending.borrow().is_none() {
            self.slider.render(value);
        }
    }
}

struct EqBandControls {
    enabled: Switch,
    kind: DropDown,
    gain: DspSlider,
    frequency: DspSlider,
    q: DspSlider,
}

pub struct DspUi {
    window: ApplicationWindow,
    syncing: Rc<Cell<bool>>,
    clipguard_mode: DropDown,
    expander: Switch,
    expander_threshold: DspSlider,
    expander_ratio: DspSlider,
    expander_attack: DspSlider,
    expander_release: DspSlider,
    expander_max_reduction: DspSlider,
    compressor: Switch,
    compressor_threshold: DspSlider,
    compressor_ratio: DspSlider,
    compressor_attack: DspSlider,
    compressor_release: DspSlider,
    compressor_makeup_gain: DspSlider,
    voice_tune: Switch,
    voice_tune_strength: DspSlider,
    eq_enabled: Switch,
    bands: Vec<EqBandControls>,
}

impl DspUi {
    pub fn new(parent: &ApplicationWindow, sender: CommandSender, initial_page: u32) -> Rc<Self> {
        let window = ApplicationWindow::builder()
            .transient_for(parent)
            .modal(true)
            .destroy_with_parent(true)
            .title("WaveLite — Wave FX")
            .width_request(1000)
            .default_width(1000)
            .build();

        let clipguard_mode = DropDown::new(
            Some(StringList::new(&["Enabled", "Bypassed", "Hard clip"])),
            None::<&gtk::Expression>,
        );
        let clipguard_row = value_row("Mode", &clipguard_mode);
        let (clipguard_group, clipguard_rows) = control_group("Clipguard 2.0");
        clipguard_rows.append(&clipguard_row);

        let (voice_tune_row, voice_tune) = switch_row("Voice tune");
        let voice_tune_strength = DspSlider::new("Strength", 0.0, 100.0, 1.0, 0, "%");
        let (voice_group, voice_rows) = control_group("Voice tune");
        append_controls(
            &voice_rows,
            &[&voice_tune_row, &voice_tune_strength.slider.row],
        );

        let (expander_row, expander) = switch_row("Enable");
        let expander_threshold = DspSlider::new("Threshold", -60.0, -1.0, 1.0, 0, "dB");
        let expander_ratio = DspSlider::new("Ratio", 1.0, 3.0, 0.1, 1, ":1");
        let expander_attack = DspSlider::new("Attack", 1.0, 200.0, 1.0, 0, "ms");
        let expander_release = DspSlider::new("Release", 10.0, 1000.0, 1.0, 0, "ms");
        let expander_max_reduction = DspSlider::new("Maximum reduction", 0.0, 60.0, 1.0, 0, "dB");
        let (expander_group, expander_rows) = control_group("Expander");
        append_controls(
            &expander_rows,
            &[
                &expander_row,
                &expander_threshold.slider.row,
                &expander_ratio.slider.row,
                &expander_attack.slider.row,
                &expander_release.slider.row,
                &expander_max_reduction.slider.row,
            ],
        );

        let (compressor_row, compressor) = switch_row("Enable");
        let compressor_threshold = DspSlider::new("Threshold", -60.0, -1.0, 1.0, 0, "dB");
        let compressor_ratio = DspSlider::new("Ratio", 1.0, 20.0, 1.0, 0, ":1");
        let compressor_attack = DspSlider::new("Attack", 1.0, 300.0, 1.0, 0, "ms");
        let compressor_release = DspSlider::new("Release", 50.0, 1000.0, 1.0, 0, "ms");
        let compressor_makeup_gain = DspSlider::new("Makeup gain", 0.0, 20.0, 1.0, 0, "dB");
        let (compressor_group, compressor_rows) = control_group("Compressor");
        append_controls(
            &compressor_rows,
            &[
                &compressor_row,
                &compressor_threshold.slider.row,
                &compressor_ratio.slider.row,
                &compressor_attack.slider.row,
                &compressor_release.slider.row,
                &compressor_makeup_gain.slider.row,
            ],
        );

        let left = Box::new(Orientation::Vertical, 12);
        left.set_hexpand(true);
        left.append(&clipguard_group);
        left.append(&voice_group);
        left.append(&expander_group);
        let right = Box::new(Orientation::Vertical, 12);
        right.set_hexpand(true);
        right.append(&compressor_group);
        let dynamics_page = Grid::new();
        dynamics_page.set_row_spacing(12);
        dynamics_page.set_column_spacing(12);
        dynamics_page.set_margin_top(12);
        dynamics_page.set_margin_bottom(12);
        dynamics_page.set_margin_start(12);
        dynamics_page.set_margin_end(12);
        dynamics_page.attach(&left, 0, 0, 1, 1);
        dynamics_page.attach(&right, 1, 0, 1, 1);

        let (eq_enabled_row, eq_enabled) = switch_row("Enable four-band EQ");
        let (eq_master_group, eq_master_rows) = control_group("Equalizer");
        eq_master_rows.append(&eq_enabled_row);
        let mut bands = Vec::new();
        let band_grid = Grid::new();
        band_grid.set_row_spacing(12);
        band_grid.set_column_spacing(12);
        for index in 0..4 {
            let (enabled_row, enabled) = switch_row("Band enabled");
            let kind = DropDown::new(
                Some(StringList::new(&EQ_BAND_TYPE_LABELS)),
                None::<&gtk::Expression>,
            );
            let kind_row = value_row("Type", &kind);
            let gain = DspSlider::new("Gain", -12.0, 12.0, 0.25, 2, "dB");
            let frequency = DspSlider::new("Frequency", 20.0, 20_000.0, 1.0, 0, "Hz");
            let q = DspSlider::new("Q", 0.1, 10.0, 0.01, 2, "");
            let (group, rows) = control_group(&format!("Band {}", index + 1));
            append_controls(
                &rows,
                &[
                    &enabled_row,
                    &kind_row,
                    &gain.slider.row,
                    &frequency.slider.row,
                    &q.slider.row,
                ],
            );
            group.set_hexpand(true);
            band_grid.attach(&group, index % 2, index / 2, 1, 1);
            bands.push(EqBandControls {
                enabled,
                kind,
                gain,
                frequency,
                q,
            });
        }
        let eq_page = Box::new(Orientation::Vertical, 12);
        eq_page.set_margin_top(12);
        eq_page.set_margin_bottom(12);
        eq_page.set_margin_start(12);
        eq_page.set_margin_end(12);
        eq_page.append(&eq_master_group);
        eq_page.append(&band_grid);

        let pages = Stack::new();
        pages.set_vhomogeneous(false);
        pages.add_titled(&dynamics_page, Some("dynamics"), "Dynamics");
        pages.add_titled(&eq_page, Some("eq"), "Four-band EQ");
        pages.set_visible_child_name(if initial_page == 1 { "eq" } else { "dynamics" });
        let switcher = StackSwitcher::new();
        switcher.set_stack(Some(&pages));
        let root = Box::new(Orientation::Vertical, 12);
        root.append(&switcher);
        root.append(&pages);
        window.set_child(Some(&root));
        pages.connect_visible_child_name_notify({
            let window = window.clone();
            move |_| window.set_default_size(window.width(), 1)
        });

        let syncing = Rc::new(Cell::new(false));
        let ui = Rc::new(Self {
            window,
            syncing,
            clipguard_mode,
            expander,
            expander_threshold,
            expander_ratio,
            expander_attack,
            expander_release,
            expander_max_reduction,
            compressor,
            compressor_threshold,
            compressor_ratio,
            compressor_attack,
            compressor_release,
            compressor_makeup_gain,
            voice_tune,
            voice_tune_strength,
            eq_enabled,
            bands,
        });
        ui.bind(sender);
        ui
    }

    pub fn present(&self) {
        self.window.present();
        self.window.set_default_size(self.window.width(), 1);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn visible_window(&self) -> Option<gtk::Window> {
        self.window.is_visible().then(|| self.window.clone().into())
    }

    pub fn render_config(&self, config: &Wave3Mk2Config) {
        let view = config.view();
        self.syncing.set(true);
        if let Some(mode) = config.clipguard_mode() {
            self.clipguard_mode.set_selected(match mode {
                ClipguardMode::Enabled => 0,
                ClipguardMode::Bypassed => 1,
                ClipguardMode::HardClip => 2,
            });
        }
        self.expander.set_active(config.expander());
        self.expander_threshold
            .render(view.expander_settings.threshold_db);
        self.expander_ratio.render(view.expander_settings.ratio);
        self.expander_attack
            .render(f64::from(view.expander_settings.attack_ms));
        self.expander_release
            .render(f64::from(view.expander_settings.release_ms));
        self.expander_max_reduction
            .render(f64::from(view.expander_settings.max_reduction_db));
        self.compressor.set_active(config.compressor());
        self.compressor_threshold
            .render(view.compressor_settings.threshold_db);
        self.compressor_ratio.render(view.compressor_settings.ratio);
        self.compressor_attack
            .render(f64::from(view.compressor_settings.attack_ms));
        self.compressor_release
            .render(f64::from(view.compressor_settings.release_ms));
        self.compressor_makeup_gain
            .render(f64::from(view.compressor_settings.makeup_gain_db));
        self.voice_tune.set_active(config.voice_tune());
        self.voice_tune_strength
            .render(f64::from(config.voice_tune_strength_percent()));
        self.eq_enabled.set_active(config.eq_enabled());
        for (index, band) in self.bands.iter().enumerate() {
            let view = &view.eq.bands[index];
            band.enabled.set_active(view.enabled);
            if let Some(selected) = EQ_BAND_TYPES
                .iter()
                .position(|kind| kind.as_str() == view.kind)
            {
                band.kind.set_selected(selected as u32);
            }
            band.gain.render(view.gain_db);
            band.frequency.render(f64::from(view.frequency_hz));
            band.q.render(view.q);
        }
        self.syncing.set(false);
    }

    fn bind(&self, sender: CommandSender) {
        self.clipguard_mode.connect_selected_notify({
            let sender = sender.clone();
            let syncing = self.syncing.clone();
            move |dropdown| {
                if syncing.get() {
                    return;
                }
                let mode = match dropdown.selected() {
                    0 => ClipguardMode::Enabled,
                    1 => ClipguardMode::Bypassed,
                    2 => ClipguardMode::HardClip,
                    _ => return,
                };
                sender.send(Command::Set(Setting::ClipguardMode(mode)));
            }
        });
        bind_switch(
            &self.expander,
            &sender,
            self.syncing.clone(),
            Setting::Expander,
        );
        bind_dsp_slider(&self.expander_threshold, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                expander_threshold_db: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(&self.expander_ratio, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                expander_ratio: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(&self.expander_attack, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                expander_attack_ms: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(&self.expander_release, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                expander_release_ms: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(
            &self.expander_max_reduction,
            &sender,
            &self.syncing,
            |value| {
                Setting::Dynamics(DynamicsUpdate {
                    expander_max_reduction_db: Some(value),
                    ..DynamicsUpdate::default()
                })
            },
        );
        bind_switch(
            &self.compressor,
            &sender,
            self.syncing.clone(),
            Setting::Compressor,
        );
        bind_dsp_slider(
            &self.compressor_threshold,
            &sender,
            &self.syncing,
            |value| {
                Setting::Dynamics(DynamicsUpdate {
                    compressor_threshold_db: Some(value),
                    ..DynamicsUpdate::default()
                })
            },
        );
        bind_dsp_slider(&self.compressor_ratio, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                compressor_ratio: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(&self.compressor_attack, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                compressor_attack_ms: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(&self.compressor_release, &sender, &self.syncing, |value| {
            Setting::Dynamics(DynamicsUpdate {
                compressor_release_ms: Some(value),
                ..DynamicsUpdate::default()
            })
        });
        bind_dsp_slider(
            &self.compressor_makeup_gain,
            &sender,
            &self.syncing,
            |value| {
                Setting::Dynamics(DynamicsUpdate {
                    compressor_makeup_gain_db: Some(value),
                    ..DynamicsUpdate::default()
                })
            },
        );
        bind_switch(
            &self.voice_tune,
            &sender,
            self.syncing.clone(),
            Setting::VoiceTune,
        );
        bind_dsp_slider(
            &self.voice_tune_strength,
            &sender,
            &self.syncing,
            Setting::VoiceTuneStrength,
        );
        bind_switch(
            &self.eq_enabled,
            &sender,
            self.syncing.clone(),
            Setting::EqEnabled,
        );
        for (index, band) in self.bands.iter().enumerate() {
            let band_index = index + 1;
            bind_switch(
                &band.enabled,
                &sender,
                self.syncing.clone(),
                move |enabled| Setting::EqBand {
                    index: band_index,
                    update: EqBandUpdate {
                        enabled: Some(enabled),
                        ..EqBandUpdate::default()
                    },
                },
            );
            band.kind.connect_selected_notify({
                let sender = sender.clone();
                let syncing = self.syncing.clone();
                move |dropdown| {
                    if syncing.get() {
                        return;
                    }
                    let Some(kind) = EQ_BAND_TYPES.get(dropdown.selected() as usize) else {
                        return;
                    };
                    sender.send(Command::Set(Setting::EqBand {
                        index: band_index,
                        update: EqBandUpdate {
                            kind: Some(*kind),
                            ..EqBandUpdate::default()
                        },
                    }));
                }
            });
            bind_dsp_slider(&band.gain, &sender, &self.syncing, move |value| {
                Setting::EqBand {
                    index: band_index,
                    update: EqBandUpdate {
                        gain_db: Some(value),
                        ..EqBandUpdate::default()
                    },
                }
            });
            bind_dsp_slider(&band.frequency, &sender, &self.syncing, move |value| {
                Setting::EqBand {
                    index: band_index,
                    update: EqBandUpdate {
                        frequency_hz: Some(value),
                        ..EqBandUpdate::default()
                    },
                }
            });
            bind_dsp_slider(&band.q, &sender, &self.syncing, move |value| {
                Setting::EqBand {
                    index: band_index,
                    update: EqBandUpdate {
                        q: Some(value),
                        ..EqBandUpdate::default()
                    },
                }
            });
        }
    }
}

fn bind_dsp_slider(
    control: &DspSlider,
    sender: &CommandSender,
    syncing: &Rc<Cell<bool>>,
    setting: impl Fn(f64) -> Setting + Clone + 'static,
) {
    bind_slider(
        &control.slider,
        sender,
        syncing.clone(),
        control.pending.clone(),
        setting,
    );
}
