import AppKit
import WaveLiteKit

final class EqBandControls: NSStackView {
    let enabledSwitch = NSSwitch()
    let kind = NSPopUpButton()
    let gain = SliderControl(title: "Gain", minimum: -12, maximum: 12, digits: 2, unit: "dB")
    let frequency = SliderControl(title: "Frequency", minimum: 20, maximum: 20_000, digits: 0, unit: "Hz")
    let q = SliderControl(title: "Q", minimum: 0.1, maximum: 10, digits: 2, unit: "")
    var onUpdate: ((EqBandUpdate) -> Void)?
    private var syncing = false

    init(index: Int) {
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 6

        let heading = NSTextField(labelWithString: "Band \(index)")
        heading.font = .boldSystemFont(ofSize: 14)
        addArrangedSubview(heading)

        enabledSwitch.target = self
        enabledSwitch.action = #selector(changeEnabled)
        enabledSwitch.setAccessibilityLabel("Band \(index) enabled")
        enabledSwitch.setAccessibilityIdentifier("eq-band-\(index)-enabled")
        let enabledRow = valueRow("Band enabled", control: enabledSwitch)

        kind.addItems(withTitles: ["High pass", "Low shelf", "Bell", "Notch", "High shelf", "Low pass"])
        kind.target = self
        kind.action = #selector(changeKind)
        kind.setAccessibilityLabel("Band \(index) type")
        kind.setAccessibilityIdentifier("eq-band-\(index)-kind")
        let kindRow = valueRow("Type", control: kind)

        gain.slider.setAccessibilityIdentifier("eq-band-\(index)-gain")
        gain.onCommit = { [weak self] value in
            self?.onUpdate?(EqBandUpdate(gainDb: value))
        }
        frequency.slider.setAccessibilityIdentifier("eq-band-\(index)-frequency")
        frequency.onCommit = { [weak self] value in
            self?.onUpdate?(EqBandUpdate(frequencyHz: value))
        }
        q.slider.setAccessibilityIdentifier("eq-band-\(index)-q")
        q.onCommit = { [weak self] value in
            self?.onUpdate?(EqBandUpdate(q: value))
        }
        let list = makeSettingList(rows: [enabledRow, kindRow, gain, frequency, q])
        addArrangedSubview(list)
        heading.widthAnchor.constraint(equalTo: widthAnchor).isActive = true
        list.widthAnchor.constraint(equalTo: widthAnchor).isActive = true
    }

    required init?(coder: NSCoder) {
        fatalError("EqBandControls does not support Interface Builder")
    }

    var controls: [NSControl] {
        [enabledSwitch, kind, gain.slider, frequency.slider, q.slider]
    }

    func render(_ view: EqBandView) {
        syncing = true
        enabledSwitch.state = view.enabled ? .on : .off
        let kinds: [EqBandType] = [.highPass, .lowShelf, .bell, .notch, .highShelf, .lowPass]
        if let kindValue = view.kind, let selection = kinds.firstIndex(of: kindValue) {
            kind.selectItem(at: selection)
        }
        gain.render(view.gainDb)
        frequency.render(Double(view.frequencyHz))
        q.render(view.q)
        syncing = false
    }

    func cancelPendingCommits() {
        gain.cancelPendingCommit()
        frequency.cancelPendingCommit()
        q.cancelPendingCommit()
    }

    @objc private func changeEnabled() {
        guard !syncing else { return }
        onUpdate?(EqBandUpdate(enabled: enabledSwitch.state == .on))
    }

    @objc private func changeKind() {
        guard !syncing else { return }
        let kinds: [EqBandType] = [.highPass, .lowShelf, .bell, .notch, .highShelf, .lowPass]
        guard kinds.indices.contains(kind.indexOfSelectedItem) else { return }
        onUpdate?(EqBandUpdate(kind: kinds[kind.indexOfSelectedItem]))
    }
}

final class DspViewController: NSViewController {
    private weak var mainViewController: MainViewController?
    private let tabs = NSTabView()
    private let tabSelector = NSSegmentedControl(labels: ["Dynamics", "Four-band EQ"], trackingMode: .selectOne, target: nil, action: nil)
    private var window: NSWindow?
    private var syncing = false
    private var controlsEnabled = false

    private let clipguardMode = NSPopUpButton()
    private let voiceTune = NSSwitch()
    private let voiceStrength = SliderControl(title: "Strength", minimum: 0, maximum: 100, digits: 0, unit: "%")
    private let expander = NSSwitch()
    private let expanderThreshold = SliderControl(title: "Threshold", minimum: -60, maximum: -1, digits: 0, unit: "dB")
    private let expanderRatio = SliderControl(title: "Ratio", minimum: 1, maximum: 3, digits: 1, unit: ":1")
    private let expanderAttack = SliderControl(title: "Attack", minimum: 1, maximum: 200, digits: 0, unit: "ms")
    private let expanderRelease = SliderControl(title: "Release", minimum: 10, maximum: 1000, digits: 0, unit: "ms")
    private let expanderMaxReduction = SliderControl(title: "Maximum reduction", minimum: 0, maximum: 60, digits: 0, unit: "dB")
    private let compressor = NSSwitch()
    private let compressorThreshold = SliderControl(title: "Threshold", minimum: -60, maximum: -1, digits: 0, unit: "dB")
    private let compressorRatio = SliderControl(title: "Ratio", minimum: 1, maximum: 20, digits: 1, unit: ":1")
    private let compressorAttack = SliderControl(title: "Attack", minimum: 1, maximum: 300, digits: 0, unit: "ms")
    private let compressorRelease = SliderControl(title: "Release", minimum: 50, maximum: 1000, digits: 0, unit: "ms")
    private let compressorMakeupGain = SliderControl(title: "Makeup gain", minimum: 0, maximum: 20, digits: 0, unit: "dB")
    private let eqEnabled = NSSwitch()
    private let bands = (1...4).map(EqBandControls.init(index:))

    init(parent: MainViewController) {
        self.mainViewController = parent
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("DspViewController does not support Interface Builder")
    }

    override func loadView() {
        view = NSView(frame: NSRect(x: 0, y: 0, width: waveFxWindowContentWidth, height: 1))
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        configureControls()
        tabs.tabViewType = .noTabsNoBorder
        tabSelector.selectedSegment = 0
        tabSelector.target = self
        tabSelector.action = #selector(changeTab)
        tabSelector.setAccessibilityIdentifier("wave-fx-tabs")

        let dynamics = NSStackView(views: [dynamicsLeftColumn(), dynamicsRightColumn()])
        dynamics.orientation = .horizontal
        dynamics.alignment = .top
        dynamics.distribution = .fillEqually
        dynamics.spacing = 12
        dynamics.edgeInsets = NSEdgeInsets(
            top: windowMargin,
            left: windowMargin,
            bottom: windowMargin,
            right: windowMargin
        )

        let eqMaster = makeGroup(title: "Equalizer", rows: [valueRow("Enable four-band EQ", control: eqEnabled)])
        let eqTop = NSStackView(views: [bands[0], bands[1]])
        eqTop.orientation = .horizontal
        eqTop.alignment = .top
        eqTop.distribution = .fillEqually
        eqTop.spacing = 12
        let eqBottom = NSStackView(views: [bands[2], bands[3]])
        eqBottom.orientation = .horizontal
        eqBottom.alignment = .top
        eqBottom.distribution = .fillEqually
        eqBottom.spacing = 12
        let eq = NSStackView(views: [eqMaster, eqTop, eqBottom])
        eq.orientation = .vertical
        eq.alignment = .leading
        eq.spacing = 12
        eq.edgeInsets = NSEdgeInsets(
            top: windowMargin,
            left: windowMargin,
            bottom: windowMargin,
            right: windowMargin
        )
        for row in [eqMaster, eqTop, eqBottom] {
            row.widthAnchor.constraint(equalTo: eq.widthAnchor, constant: -(2 * windowMargin)).isActive = true
        }

        let dynamicsItem = NSTabViewItem(identifier: "dynamics")
        dynamicsItem.label = "Dynamics"
        dynamicsItem.view = tabPage(containing: dynamics)
        tabs.addTabViewItem(dynamicsItem)
        let eqItem = NSTabViewItem(identifier: "eq")
        eqItem.label = "Four-band EQ"
        eqItem.view = tabPage(containing: eq)
        tabs.addTabViewItem(eqItem)

        tabSelector.translatesAutoresizingMaskIntoConstraints = false
        tabs.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(tabSelector)
        view.addSubview(tabs)
        NSLayoutConstraint.activate([
            view.widthAnchor.constraint(equalToConstant: waveFxWindowContentWidth),
            tabSelector.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: windowMargin),
            tabSelector.topAnchor.constraint(equalTo: view.topAnchor, constant: windowMargin),
            tabs.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            tabs.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            tabs.topAnchor.constraint(equalTo: tabSelector.bottomAnchor, constant: 8),
            tabs.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        setEnabled(controlsEnabled)
    }

    var isWindowVisible: Bool {
        window?.isVisible == true
    }

    func present(page: Int) {
        _ = view
        let page = max(0, min(page, 1))
        tabs.selectTabViewItem(at: page)
        tabSelector.selectedSegment = page
        if window == nil {
            let window = NSWindow(
                contentRect: view.frame,
                styleMask: [.titled, .closable, .miniaturizable],
                backing: .buffered,
                defer: false
            )
            window.title = "WaveLite — Wave FX"
            window.contentViewController = self
            window.isReleasedWhenClosed = false
            self.window = window
            fitWindowToContent()
            window.center()
        } else {
            fitWindowToContent()
        }
        window?.makeKeyAndOrderFront(nil)
    }

    private func fitWindowToContent() {
        guard let window else { return }
        view.layoutSubtreeIfNeeded()
        let pageHeight = tabs.selectedTabViewItem?.view?.fittingSize.height ?? 0
        let height = windowMargin + tabSelector.fittingSize.height + 8 + pageHeight
        window.setContentSize(NSSize(width: waveFxWindowContentWidth, height: height))
    }

    func hide() {
        window?.orderOut(nil)
    }

    func setEnabled(_ enabled: Bool) {
        controlsEnabled = enabled
        for control in allControls() {
            control.isEnabled = enabled
        }
    }

    func cancelPendingCommits() {
        for slider in sliders() {
            slider.cancelPendingCommit()
        }
        for band in bands {
            band.cancelPendingCommits()
        }
    }

    func render(_ config: DeviceConfig) {
        guard mainViewController?.hasPendingWrites == false, !syncing else { return }
        guard let clipguardModeValue = config.clipguardMode,
              let voiceTuneValue = config.voiceTune,
              let voiceStrengthValue = config.voiceTuneStrengthPercent,
              let expanderValue = config.expander,
              let expanderSettings = config.expanderSettings,
              let compressorValue = config.compressor,
              let compressorSettings = config.compressorSettings,
              let eq = config.eq
        else { return }

        syncing = true
        let modes: [ClipguardMode] = [.enabled, .bypassed, .hardClip]
        if let selection = modes.firstIndex(of: clipguardModeValue) {
            clipguardMode.selectItem(at: selection)
        }
        voiceTune.state = voiceTuneValue ? .on : .off
        voiceStrength.render(Double(voiceStrengthValue))
        expander.state = expanderValue ? .on : .off
        expanderThreshold.render(expanderSettings.thresholdDb)
        expanderRatio.render(expanderSettings.ratio)
        expanderAttack.render(Double(expanderSettings.attackMs))
        expanderRelease.render(Double(expanderSettings.releaseMs))
        expanderMaxReduction.render(Double(expanderSettings.maxReductionDb))
        compressor.state = compressorValue ? .on : .off
        compressorThreshold.render(compressorSettings.thresholdDb)
        compressorRatio.render(compressorSettings.ratio)
        compressorAttack.render(Double(compressorSettings.attackMs))
        compressorRelease.render(Double(compressorSettings.releaseMs))
        compressorMakeupGain.render(Double(compressorSettings.makeupGainDb))
        eqEnabled.state = eq.enabled ? .on : .off
        for (band, view) in zip(bands, eq.bands) {
            band.render(view)
        }
        syncing = false
    }

    private func configureControls() {
        clipguardMode.addItems(withTitles: ["Enabled", "Bypassed", "Hard clip"])
        clipguardMode.target = self
        clipguardMode.action = #selector(changeClipguardMode)
        clipguardMode.setAccessibilityIdentifier("clipguard-mode")

        for (control, identifier) in [
            (voiceTune, "voice-tune"),
            (expander, "expander"),
            (compressor, "compressor"),
            (eqEnabled, "eq-enabled"),
        ] {
            control.target = self
            control.action = #selector(changeSwitch(_:))
            control.setAccessibilityIdentifier(identifier)
        }

        voiceStrength.slider.setAccessibilityIdentifier("voice-tune-strength")
        voiceStrength.onCommit = { [weak self] value in
            self?.mainViewController?.applySetting(.voiceTuneStrength(value))
        }
        bindDynamics(expanderThreshold, identifier: "expander-threshold") { DynamicsUpdate(expanderThresholdDb: $0) }
        bindDynamics(expanderRatio, identifier: "expander-ratio") { DynamicsUpdate(expanderRatio: $0) }
        bindDynamics(expanderAttack, identifier: "expander-attack") { DynamicsUpdate(expanderAttackMs: $0) }
        bindDynamics(expanderRelease, identifier: "expander-release") { DynamicsUpdate(expanderReleaseMs: $0) }
        bindDynamics(expanderMaxReduction, identifier: "expander-max-reduction") { DynamicsUpdate(expanderMaxReductionDb: $0) }
        bindDynamics(compressorThreshold, identifier: "compressor-threshold") { DynamicsUpdate(compressorThresholdDb: $0) }
        bindDynamics(compressorRatio, identifier: "compressor-ratio") { DynamicsUpdate(compressorRatio: $0) }
        bindDynamics(compressorAttack, identifier: "compressor-attack") { DynamicsUpdate(compressorAttackMs: $0) }
        bindDynamics(compressorRelease, identifier: "compressor-release") { DynamicsUpdate(compressorReleaseMs: $0) }
        bindDynamics(compressorMakeupGain, identifier: "compressor-makeup-gain") { DynamicsUpdate(compressorMakeupGainDb: $0) }

        for (offset, band) in bands.enumerated() {
            band.onUpdate = { [weak self] update in
                self?.mainViewController?.applySetting(.eqBand(index: UInt8(offset + 1), update: update))
            }
        }
    }

    private func bindDynamics(_ slider: SliderControl, identifier: String, update: @escaping (Double) -> DynamicsUpdate) {
        slider.slider.setAccessibilityIdentifier(identifier)
        slider.onCommit = { [weak self] value in
            self?.mainViewController?.applySetting(.dynamics(update(value)))
        }
    }

    private func dynamicsLeftColumn() -> NSView {
        let clipguard = makeGroup(title: "Clipguard 2.0", rows: [valueRow("Mode", control: clipguardMode)])
        let voice = makeGroup(title: "Voice tune", rows: [valueRow("Voice tune", control: voiceTune), voiceStrength])
        let expanderGroup = makeGroup(title: "Expander", rows: [
            valueRow("Enable", control: expander),
            expanderThreshold,
            expanderRatio,
            expanderAttack,
            expanderRelease,
            expanderMaxReduction,
        ])
        let column = NSStackView(views: [clipguard, voice, expanderGroup])
        column.orientation = .vertical
        column.spacing = 12
        for group in [clipguard, voice, expanderGroup] {
            group.widthAnchor.constraint(equalTo: column.widthAnchor).isActive = true
        }
        return column
    }

    private func dynamicsRightColumn() -> NSView {
        let compressorGroup = makeGroup(title: "Compressor", rows: [
            valueRow("Enable", control: compressor),
            compressorThreshold,
            compressorRatio,
            compressorAttack,
            compressorRelease,
            compressorMakeupGain,
        ])
        let column = NSView()
        compressorGroup.translatesAutoresizingMaskIntoConstraints = false
        column.addSubview(compressorGroup)
        NSLayoutConstraint.activate([
            compressorGroup.leadingAnchor.constraint(equalTo: column.leadingAnchor),
            compressorGroup.trailingAnchor.constraint(equalTo: column.trailingAnchor),
            compressorGroup.topAnchor.constraint(equalTo: column.topAnchor),
            compressorGroup.bottomAnchor.constraint(lessThanOrEqualTo: column.bottomAnchor),
        ])
        return column
    }

    private func tabPage(containing content: NSView) -> NSView {
        let page = NSView()
        content.translatesAutoresizingMaskIntoConstraints = false
        page.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: page.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: page.trailingAnchor),
            content.topAnchor.constraint(equalTo: page.topAnchor),
            content.bottomAnchor.constraint(lessThanOrEqualTo: page.bottomAnchor),
        ])
        return page
    }

    private func allControls() -> [NSControl] {
        [clipguardMode, voiceTune, expander, compressor, eqEnabled]
            + sliders().map(\.slider)
            + bands.flatMap(\.controls)
    }

    private func sliders() -> [SliderControl] {
        [
            voiceStrength,
            expanderThreshold,
            expanderRatio,
            expanderAttack,
            expanderRelease,
            expanderMaxReduction,
            compressorThreshold,
            compressorRatio,
            compressorAttack,
            compressorRelease,
            compressorMakeupGain,
        ]
    }

    @objc private func changeTab() {
        tabs.selectTabViewItem(at: tabSelector.selectedSegment)
        fitWindowToContent()
    }

    @objc private func changeClipguardMode() {
        guard !syncing else { return }
        let modes: [ClipguardMode] = [.enabled, .bypassed, .hardClip]
        guard modes.indices.contains(clipguardMode.indexOfSelectedItem) else { return }
        mainViewController?.applySetting(.clipguardMode(modes[clipguardMode.indexOfSelectedItem]))
    }

    @objc private func changeSwitch(_ sender: NSSwitch) {
        guard !syncing else { return }
        let enabled = sender.state == .on
        if sender === voiceTune {
            mainViewController?.applySetting(.voiceTune(enabled))
        } else if sender === expander {
            mainViewController?.applySetting(.expander(enabled))
        } else if sender === compressor {
            mainViewController?.applySetting(.compressor(enabled))
        } else if sender === eqEnabled {
            mainViewController?.applySetting(.eqEnabled(enabled))
        }
    }
}


extension DspViewController {
    func demoSteps() -> [DemoStep] {
        var steps = [
            DemoStep(name: "Clipguard-mode write", action: {
                self.present(page: 0)
                self.clipguardMode.selectItem(at: 1)
                self.trigger(self.clipguardMode)
            }, matches: { $0.clipguardMode == .bypassed }),
            DemoStep(name: "voice-tune write", action: {
                self.voiceTune.state = .off
                self.trigger(self.voiceTune)
            }, matches: { $0.voiceTune == false }),
            DemoStep(name: "voice-tune-strength write", action: {
                self.set(self.voiceStrength, value: 65)
            }, matches: { $0.voiceTuneStrengthPercent == 65 }),
            DemoStep(name: "expander-enable write", action: {
                self.expander.state = .on
                self.trigger(self.expander)
            }, matches: { $0.expander == true }),
            DemoStep(name: "expander-settings write", writeCount: 5, action: {
                self.present(page: 0)
                self.set(self.expanderThreshold, value: -40)
                self.set(self.expanderRatio, value: 2.5)
                self.set(self.expanderAttack, value: 20)
                self.set(self.expanderRelease, value: 250)
                self.set(self.expanderMaxReduction, value: 30)
            }, matches: { config in
                guard let settings = config.expanderSettings else { return false }
                return settings.thresholdDb == -40
                    && settings.ratio == 2.5
                    && settings.attackMs == 20
                    && settings.releaseMs == 250
                    && settings.maxReductionDb == 30
            }),
            DemoStep(name: "compressor-enable write", action: {
                self.compressor.state = .off
                self.trigger(self.compressor)
            }, matches: { $0.compressor == false }),
            DemoStep(name: "compressor-settings write", writeCount: 5, action: {
                self.present(page: 0)
                self.set(self.compressorThreshold, value: -25)
                self.set(self.compressorRatio, value: 6)
                self.set(self.compressorAttack, value: 15)
                self.set(self.compressorRelease, value: 150)
                self.set(self.compressorMakeupGain, value: 8)
            }, matches: { config in
                guard let settings = config.compressorSettings else { return false }
                return settings.thresholdDb == -25
                    && settings.ratio == 6
                    && settings.attackMs == 15
                    && settings.releaseMs == 150
                    && settings.makeupGainDb == 8
            }),
            DemoStep(name: "EQ-enable write", action: {
                self.present(page: 1)
                self.eqEnabled.state = .on
                self.trigger(self.eqEnabled)
            }, matches: { $0.eq?.enabled == true }),
        ]

        let expectations = [
            (enabled: true, kind: EqBandType.notch, kindIndex: 3, gain: 4.5, frequency: 1200.0, q: 1.2),
            (enabled: false, kind: EqBandType.highShelf, kindIndex: 4, gain: -2.5, frequency: 2400.0, q: 0.8),
            (enabled: false, kind: EqBandType.highPass, kindIndex: 0, gain: 1.5, frequency: 900.0, q: 0.7),
            (enabled: false, kind: EqBandType.lowPass, kindIndex: 5, gain: -4.0, frequency: 8000.0, q: 0.5),
        ]
        for (offset, expected) in expectations.enumerated() {
            let band = bands[offset]
            steps.append(DemoStep(name: "EQ band \(offset + 1) write", writeCount: 5, action: {
                self.present(page: 1)
                band.enabledSwitch.state = expected.enabled ? .on : .off
                self.trigger(band.enabledSwitch)
                band.kind.selectItem(at: expected.kindIndex)
                self.trigger(band.kind)
                self.set(band.gain, value: expected.gain)
                self.set(band.frequency, value: expected.frequency)
                self.set(band.q, value: expected.q)
            }, matches: { config in
                guard let view = config.eq?.bands[offset] else { return false }
                let expectedQ = pow(10.0, (log10(expected.q) * 64).rounded(.toNearestOrEven) / 64)
                return view.enabled == expected.enabled
                    && view.kind == expected.kind
                    && view.gainDb == expected.gain
                    && Double(view.frequencyHz) == expected.frequency
                    && view.q == expectedQ
            }))
        }
        return steps
    }

    private func trigger(_ control: NSControl) {
        _ = control.sendAction(control.action, to: control.target)
    }

    private func set(_ slider: SliderControl, value: Double) {
        slider.slider.doubleValue = value
        trigger(slider.slider)
    }
}
