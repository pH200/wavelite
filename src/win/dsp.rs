use native_windows_gui as nwg;

use crate::wave3_mk2::{ClipguardMode, EqBandType, EqBandView, Wave3Mk2Config};

use super::set_trackbar_position;

const EQ_TYPES: [EqBandType; 6] = [
    EqBandType::HighPass,
    EqBandType::LowShelf,
    EqBandType::Bell,
    EqBandType::Notch,
    EqBandType::HighShelf,
    EqBandType::LowPass,
];

#[derive(Clone, Copy)]
pub(super) enum DspSliderId {
    VoiceStrength,
    ExpanderThreshold,
    ExpanderRatio,
    ExpanderAttack,
    ExpanderRelease,
    ExpanderMaxReduction,
    CompressorThreshold,
    CompressorRatio,
    CompressorAttack,
    CompressorRelease,
    CompressorMakeup,
    EqGain(usize),
    EqFrequency(usize),
    EqQ(usize),
}

#[derive(Default)]
struct DspSlider {
    _title: nwg::Label,
    track: nwg::TrackBar,
    value: nwg::Label,
    minimum: f64,
    factor: f64,
    digits: usize,
    unit: &'static str,
}

impl DspSlider {
    fn build<C: Into<nwg::ControlHandle> + Copy>(
        parent: C,
        title: &str,
        minimum: f64,
        maximum: f64,
        factor: f64,
        digits: usize,
        unit: &'static str,
        x: i32,
        y: i32,
    ) -> Result<Self, nwg::NwgError> {
        let mut label = nwg::Label::default();
        nwg::Label::builder()
            .text(title)
            .position((x, y))
            .size((300, 24))
            .parent(parent)
            .build(&mut label)?;
        let mut value = nwg::Label::default();
        nwg::Label::builder()
            .position((x + 380, y))
            .size((86, 24))
            .parent(parent)
            .build(&mut value)?;
        let mut track = nwg::TrackBar::default();
        nwg::TrackBar::builder()
            .range(Some(0..((maximum - minimum) * factor).round() as usize))
            .pos(Some(0))
            .flags(
                nwg::TrackBarFlags::VISIBLE
                    | nwg::TrackBarFlags::HORIZONTAL
                    | nwg::TrackBarFlags::NO_TICK,
            )
            .position((x, y + 28))
            .size((466, 34))
            .parent(parent)
            .build(&mut track)?;
        let slider = Self {
            _title: label,
            track,
            value,
            minimum,
            factor,
            digits,
            unit,
        };
        slider.update();
        Ok(slider)
    }

    fn compact_layout(&self, x: i32, y: i32) {
        self._title.set_position(x, y);
        self._title.set_size(300, 20);
        self.value.set_position(x + 380, y);
        self.value.set_size(86, 20);
        self.track.set_position(x, y + 22);
        self.track.set_size(466, 28);
    }

    fn value(&self) -> f64 {
        self.minimum + self.track.pos() as f64 / self.factor
    }

    fn render(&self, value: f64) {
        let position = ((value - self.minimum) * self.factor).round().max(0.0) as usize;
        set_trackbar_position(&self.track, position);
        self.update();
    }

    fn update(&self) {
        let value = self.value();
        let digits = self.digits;
        self.value
            .set_text(&format!("{value:.digits$}{}", self.unit));
    }
}

#[derive(Default)]
struct EqBandControls {
    labels: Vec<nwg::Label>,
    enabled: nwg::CheckBox,
    kind: nwg::ComboBox<String>,
    gain: DspSlider,
    frequency: DspSlider,
    q: DspSlider,
}

impl EqBandControls {
    fn build(parent: &nwg::Tab, index: usize, x: i32, y: i32) -> Result<Self, nwg::NwgError> {
        let mut band = Self::default();
        band.labels.push(label(
            parent,
            &format!("Band {}", index + 1),
            (x, y),
            (220, 24),
        )?);
        checkbox(parent, "Band enabled", (x, y + 26), &mut band.enabled)?;
        band.labels
            .push(label(parent, "Type", (x, y + 56), (160, 20))?);
        nwg::ComboBox::builder()
            .collection(vec![
                "High pass".to_owned(),
                "Low shelf".to_owned(),
                "Bell".to_owned(),
                "Notch".to_owned(),
                "High shelf".to_owned(),
                "Low pass".to_owned(),
            ])
            .selected_index(Some(2))
            .position((x + 180, y + 52))
            .size((286, 28))
            .enabled(false)
            .parent(parent)
            .build(&mut band.kind)?;
        band.gain = DspSlider::build(parent, "Gain", -12.0, 12.0, 4.0, 2, " dB", x, y + 106)?;
        band.frequency = DspSlider::build(
            parent,
            "Frequency",
            20.0,
            20_000.0,
            1.0,
            0,
            " Hz",
            x,
            y + 178,
        )?;
        band.q = DspSlider::build(parent, "Q", 0.1, 10.0, 100.0, 2, "", x, y + 250)?;
        band.gain.compact_layout(x, y + 88);
        band.frequency.compact_layout(x, y + 146);
        band.q.compact_layout(x, y + 204);
        Ok(band)
    }

    fn render(&self, view: &EqBandView) {
        self.enabled.set_check_state(check(view.enabled));
        if let Some(selected) = EQ_TYPES.iter().position(|kind| kind.as_str() == view.kind) {
            self.kind.set_selection(Some(selected));
        }
        self.gain.render(view.gain_db);
        self.frequency.render(f64::from(view.frequency_hz));
        self.q.render(view.q);
    }
}

#[derive(Default)]
pub(super) struct DspControls {
    labels: Vec<nwg::Label>,
    pub tabs: nwg::TabsContainer,
    dynamics_tab: nwg::Tab,
    eq_tab: nwg::Tab,
    pub clipguard_mode: nwg::ComboBox<String>,
    pub voice_tune: nwg::CheckBox,
    voice_strength: DspSlider,
    pub expander: nwg::CheckBox,
    expander_threshold: DspSlider,
    expander_ratio: DspSlider,
    expander_attack: DspSlider,
    expander_release: DspSlider,
    expander_max_reduction: DspSlider,
    pub compressor: nwg::CheckBox,
    compressor_threshold: DspSlider,
    compressor_ratio: DspSlider,
    compressor_attack: DspSlider,
    compressor_release: DspSlider,
    compressor_makeup: DspSlider,
    pub eq_enabled: nwg::CheckBox,
    bands: [EqBandControls; 4],
}

impl DspControls {
    pub fn build(parent: &nwg::Window) -> Result<Self, nwg::NwgError> {
        let mut controls = Self::default();
        nwg::TabsContainer::builder()
            .size((990, 680))
            .position((0, 0))
            .parent(parent)
            .build(&mut controls.tabs)?;
        nwg::Tab::builder()
            .text("Dynamics")
            .parent(&controls.tabs)
            .build(&mut controls.dynamics_tab)?;
        nwg::Tab::builder()
            .text("Four-band EQ")
            .parent(&controls.tabs)
            .build(&mut controls.eq_tab)?;

        let dynamics = &controls.dynamics_tab;
        controls
            .labels
            .push(label(dynamics, "Clipguard 2.0", (12, 12), (220, 24))?);
        nwg::ComboBox::builder()
            .collection(vec![
                "Enabled".to_owned(),
                "Bypassed".to_owned(),
                "Hard clip".to_owned(),
            ])
            .selected_index(Some(0))
            .position((200, 36))
            .size((266, 32))
            .enabled(false)
            .parent(dynamics)
            .build(&mut controls.clipguard_mode)?;
        controls
            .labels
            .push(label(dynamics, "Mode", (12, 42), (160, 24))?);

        controls
            .labels
            .push(label(dynamics, "Voice tune", (12, 82), (220, 24))?);
        checkbox(dynamics, "Voice tune", (12, 112), &mut controls.voice_tune)?;
        controls.voice_strength =
            DspSlider::build(dynamics, "Strength", 0.0, 100.0, 1.0, 0, "%", 12, 154)?;

        controls
            .labels
            .push(label(dynamics, "Expander", (12, 220), (220, 24))?);
        checkbox(dynamics, "Enable", (12, 250), &mut controls.expander)?;
        controls.expander_threshold =
            DspSlider::build(dynamics, "Threshold", -60.0, -1.0, 1.0, 0, " dB", 12, 292)?;
        controls.expander_ratio =
            DspSlider::build(dynamics, "Ratio", 1.0, 3.0, 10.0, 1, ":1", 12, 364)?;
        controls.expander_attack =
            DspSlider::build(dynamics, "Attack", 1.0, 200.0, 1.0, 0, " ms", 12, 436)?;
        controls.expander_release =
            DspSlider::build(dynamics, "Release", 10.0, 1000.0, 1.0, 0, " ms", 12, 508)?;
        controls.expander_max_reduction = DspSlider::build(
            dynamics,
            "Maximum reduction",
            0.0,
            60.0,
            1.0,
            0,
            " dB",
            12,
            580,
        )?;

        let compressor = &controls.dynamics_tab;
        controls
            .labels
            .push(label(compressor, "Compressor", (506, 12), (220, 24))?);
        checkbox(compressor, "Enable", (506, 44), &mut controls.compressor)?;
        controls.compressor_threshold =
            DspSlider::build(compressor, "Threshold", -60.0, -1.0, 1.0, 0, " dB", 506, 86)?;
        controls.compressor_ratio =
            DspSlider::build(compressor, "Ratio", 1.0, 20.0, 1.0, 0, ":1", 506, 158)?;
        controls.compressor_attack =
            DspSlider::build(compressor, "Attack", 1.0, 300.0, 1.0, 0, " ms", 506, 230)?;
        controls.compressor_release =
            DspSlider::build(compressor, "Release", 50.0, 1000.0, 1.0, 0, " ms", 506, 302)?;
        controls.compressor_makeup = DspSlider::build(
            compressor,
            "Makeup gain",
            0.0,
            20.0,
            1.0,
            0,
            " dB",
            506,
            374,
        )?;

        controls
            .labels
            .push(label(&controls.eq_tab, "Equalizer", (12, 12), (180, 24))?);
        checkbox(
            &controls.eq_tab,
            "Enable four-band EQ",
            (200, 8),
            &mut controls.eq_enabled,
        )?;
        let positions = [(12, 52), (506, 52), (12, 324), (506, 324)];
        for (index, band) in controls.bands.iter_mut().enumerate() {
            let (x, y) = positions[index];
            *band = EqBandControls::build(&controls.eq_tab, index, x, y)?;
        }
        controls.set_enabled(false);
        Ok(controls)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.clipguard_mode.set_enabled(enabled);
        self.voice_tune.set_enabled(enabled);
        self.expander.set_enabled(enabled);
        self.compressor.set_enabled(enabled);
        self.eq_enabled.set_enabled(enabled);
        for band in &self.bands {
            band.enabled.set_enabled(enabled);
            band.kind.set_enabled(enabled);
        }
        for (_, slider) in self.sliders() {
            slider.track.set_enabled(enabled);
        }
    }

    pub fn render(&self, config: &Wave3Mk2Config) {
        if let Some(mode) = config.clipguard_mode() {
            self.clipguard_mode.set_selection(Some(match mode {
                ClipguardMode::Enabled => 0,
                ClipguardMode::Bypassed => 1,
                ClipguardMode::HardClip => 2,
            }));
        }
        self.voice_tune.set_check_state(check(config.voice_tune()));
        self.expander.set_check_state(check(config.expander()));
        self.compressor.set_check_state(check(config.compressor()));
        let view = config.view();
        self.voice_strength
            .render(f64::from(config.voice_tune_strength_percent()));
        self.expander_threshold
            .render(view.expander_settings.threshold_db);
        self.expander_ratio.render(view.expander_settings.ratio);
        self.expander_attack
            .render(f64::from(view.expander_settings.attack_ms));
        self.expander_release
            .render(f64::from(view.expander_settings.release_ms));
        self.expander_max_reduction
            .render(f64::from(view.expander_settings.max_reduction_db));
        self.compressor_threshold
            .render(view.compressor_settings.threshold_db);
        self.compressor_ratio.render(view.compressor_settings.ratio);
        self.compressor_attack
            .render(f64::from(view.compressor_settings.attack_ms));
        self.compressor_release
            .render(f64::from(view.compressor_settings.release_ms));
        self.compressor_makeup
            .render(f64::from(view.compressor_settings.makeup_gain_db));
        self.eq_enabled.set_check_state(check(config.eq_enabled()));
        for (band, view) in self.bands.iter().zip(&view.eq.bands) {
            band.render(view);
        }
    }

    pub fn slider_for(&self, handle: &nwg::ControlHandle) -> Option<DspSliderId> {
        self.sliders()
            .into_iter()
            .find_map(|(id, slider)| (handle == &slider.track).then_some(id))
    }

    pub fn update(&self, id: DspSliderId) {
        self.slider(id).update();
    }

    pub fn value(&self, id: DspSliderId) -> f64 {
        self.slider(id).value()
    }

    pub fn eq_band_for(&self, handle: &nwg::ControlHandle) -> Option<usize> {
        self.bands
            .iter()
            .position(|band| handle == &band.enabled)
            .map(|index| index + 1)
    }

    pub fn eq_kind_for(&self, handle: &nwg::ControlHandle) -> Option<usize> {
        self.bands
            .iter()
            .position(|band| handle == &band.kind)
            .map(|index| index + 1)
    }

    pub fn eq_band_enabled(&self, index: usize) -> bool {
        self.bands[index - 1].enabled.check_state() == nwg::CheckBoxState::Checked
    }

    pub fn eq_kind(&self, index: usize) -> Option<EqBandType> {
        self.bands[index - 1]
            .kind
            .selection()
            .and_then(|selected| EQ_TYPES.get(selected).copied())
    }

    fn slider(&self, id: DspSliderId) -> &DspSlider {
        match id {
            DspSliderId::VoiceStrength => &self.voice_strength,
            DspSliderId::ExpanderThreshold => &self.expander_threshold,
            DspSliderId::ExpanderRatio => &self.expander_ratio,
            DspSliderId::ExpanderAttack => &self.expander_attack,
            DspSliderId::ExpanderRelease => &self.expander_release,
            DspSliderId::ExpanderMaxReduction => &self.expander_max_reduction,
            DspSliderId::CompressorThreshold => &self.compressor_threshold,
            DspSliderId::CompressorRatio => &self.compressor_ratio,
            DspSliderId::CompressorAttack => &self.compressor_attack,
            DspSliderId::CompressorRelease => &self.compressor_release,
            DspSliderId::CompressorMakeup => &self.compressor_makeup,
            DspSliderId::EqGain(index) => &self.bands[index - 1].gain,
            DspSliderId::EqFrequency(index) => &self.bands[index - 1].frequency,
            DspSliderId::EqQ(index) => &self.bands[index - 1].q,
        }
    }

    fn sliders(&self) -> Vec<(DspSliderId, &DspSlider)> {
        let mut sliders = vec![
            (DspSliderId::VoiceStrength, &self.voice_strength),
            (DspSliderId::ExpanderThreshold, &self.expander_threshold),
            (DspSliderId::ExpanderRatio, &self.expander_ratio),
            (DspSliderId::ExpanderAttack, &self.expander_attack),
            (DspSliderId::ExpanderRelease, &self.expander_release),
            (
                DspSliderId::ExpanderMaxReduction,
                &self.expander_max_reduction,
            ),
            (DspSliderId::CompressorThreshold, &self.compressor_threshold),
            (DspSliderId::CompressorRatio, &self.compressor_ratio),
            (DspSliderId::CompressorAttack, &self.compressor_attack),
            (DspSliderId::CompressorRelease, &self.compressor_release),
            (DspSliderId::CompressorMakeup, &self.compressor_makeup),
        ];
        for (index, band) in self.bands.iter().enumerate() {
            let index = index + 1;
            sliders.extend([
                (DspSliderId::EqGain(index), &band.gain),
                (DspSliderId::EqFrequency(index), &band.frequency),
                (DspSliderId::EqQ(index), &band.q),
            ]);
        }
        sliders
    }
}

fn check(checked: bool) -> nwg::CheckBoxState {
    if checked {
        nwg::CheckBoxState::Checked
    } else {
        nwg::CheckBoxState::Unchecked
    }
}

fn label<C: Into<nwg::ControlHandle> + Copy>(
    parent: C,
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

fn checkbox<C: Into<nwg::ControlHandle> + Copy>(
    parent: C,
    text: &str,
    position: (i32, i32),
    output: &mut nwg::CheckBox,
) -> Result<(), nwg::NwgError> {
    nwg::CheckBox::builder()
        .text(text)
        .position(position)
        .size((220, 30))
        .enabled(false)
        .parent(parent)
        .build(output)
}
