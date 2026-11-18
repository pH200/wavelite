import AppKit
import WaveLiteKit

final class MainViewController: NSViewController {
    private let deviceName = NSTextField(labelWithString: "WaveLite")
    private let firmware = NSTextField(labelWithString: "Firmware: unavailable")
    private let serial = NSTextField(labelWithString: "Serial: ************")
    private let serialButton = NSButton()
    private let gain = SliderControl(title: "Input gain", minimum: 0, maximum: 40, digits: 1, unit: "dB")
    private let muteSwitch = NSSwitch()
    private let clipguardSwitch = NSSwitch()
    private let lowcutSwitch = NSSwitch()
    private let outputVolume = SliderControl(title: "Headphone volume", minimum: -128, maximum: 0, digits: 1, unit: "dB")
    private let monitorMix = SliderControl(title: "Monitor mix", minimum: 0, maximum: 100, digits: 0, unit: "%")
    private let dial = NSPopUpButton()
    private let dialGroup = NSStackView()
    private let dspButton = NSButton(title: "DSP & EQ…", target: nil, action: nil)
    private let dspGroup = NSStackView()
    private lazy var dsp = DspViewController(parent: self)
    private let status = NSTextField(wrappingLabelWithString: "Connecting…")
    private var worker: DeviceWorker?
    var initialWaveFxPage: Int?
    private var model: DeviceModel = .wave1
    private var serialNumber = ""
    private var serialVisible = false
    private var syncing = false
    private var connected = false
    private var pendingWrites = 0

    override func loadView() {
        view = NSView(frame: NSRect(x: 0, y: 0, width: mainWindowContentWidth, height: 1))
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        deviceName.font = .boldSystemFont(ofSize: 22)
        firmware.textColor = .secondaryLabelColor
        serial.textColor = .secondaryLabelColor

        serialButton.bezelStyle = .inline
        serialButton.isBordered = false
        serialButton.image = NSImage(systemSymbolName: "eye", accessibilityDescription: "Show serial number")
        serialButton.toolTip = "Show serial number"
        serialButton.target = self
        serialButton.action = #selector(toggleSerial)
        serialButton.setAccessibilityLabel("Show serial number")
        serialButton.setAccessibilityIdentifier("serial-toggle")

        let serialRow = NSStackView(views: [serial, serialButton])
        serialRow.orientation = .horizontal
        serialRow.alignment = .centerY
        serialRow.spacing = 8

        gain.slider.setAccessibilityIdentifier("input-gain")
        gain.onCommit = { [weak self] value in
            self?.applySetting(.gain(value))
        }
        outputVolume.slider.setAccessibilityIdentifier("output-volume")
        outputVolume.onCommit = { [weak self] value in
            self?.applySetting(.outputVolume(value))
        }
        monitorMix.slider.setAccessibilityIdentifier("monitor-mix")
        monitorMix.onCommit = { [weak self] value in
            self?.applySetting(.monitorMix(value))
        }

        let muteRow = switchRow("Mute", control: muteSwitch, identifier: "mute")
        let clipguardRow = switchRow("Clipguard", control: clipguardSwitch, identifier: "clipguard")
        let lowcutRow = switchRow("Enhanced Lowcut", control: lowcutSwitch, identifier: "lowcut")
        let microphone = makeGroup(title: "Microphone", rows: [gain, muteRow, clipguardRow, lowcutRow])
        let output = makeGroup(title: "Output", rows: [outputVolume, monitorMix])

        dial.target = self
        dial.action = #selector(changeDial)
        dial.setAccessibilityLabel("Dial function")
        dial.setAccessibilityIdentifier("dial-mode")
        let dialRow = valueRow("Function", control: dial)
        configureGroup(dialGroup, title: "Dial", rows: [dialRow])

        dspButton.target = self
        dspButton.action = #selector(presentDsp)
        dspButton.setAccessibilityIdentifier("dsp-eq-button")
        configureGroup(dspGroup, title: "Wave FX", rows: [valueRow("Processing", control: dspButton)])
        dspGroup.isHidden = true

        status.textColor = .secondaryLabelColor
        let metadataRow = NSStackView(views: [firmware, serialRow])
        metadataRow.orientation = .horizontal
        metadataRow.alignment = .centerY
        metadataRow.spacing = 12

        let device = NSStackView(views: [deviceName, metadataRow])
        device.orientation = .vertical
        device.alignment = .leading
        device.spacing = 6

        let controls = NSStackView(views: [microphone, output, dialGroup, dspGroup])
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 12

        let content = NSStackView(views: [device, controls, status])
        content.orientation = .vertical
        content.alignment = .leading
        content.spacing = 12
        content.edgeInsets = NSEdgeInsets(
            top: windowMargin,
            left: windowMargin,
            bottom: windowMargin,
            right: windowMargin
        )
        content.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(content)

        for item in [device, controls, status] {
            item.widthAnchor.constraint(
                equalTo: content.widthAnchor,
                constant: -(2 * windowMargin)
            ).isActive = true
        }
        for group in [microphone, output, dialGroup, dspGroup] {
            group.widthAnchor.constraint(equalTo: controls.widthAnchor).isActive = true
        }
        NSLayoutConstraint.activate([
            view.widthAnchor.constraint(equalToConstant: mainWindowContentWidth),
            content.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            content.topAnchor.constraint(equalTo: view.topAnchor),
            content.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        setConnected(false)
    }

    func attach(worker: DeviceWorker) {
        self.worker = worker
        worker.start()
    }

    func stop() {
        cancelPendingSliders()
        dsp.cancelPendingCommits()
        dsp.hide()
        worker?.stop()
        worker = nil
    }

    func captureView() -> NSView {
        dsp.isWindowVisible ? dsp.view : view
    }

    func fitWindowToContent() {
        guard let window = view.window else { return }
        view.layoutSubtreeIfNeeded()
        window.setContentSize(view.fittingSize)
    }

    func finishDemoTest() {
        if let initialWaveFxPage {
            dsp.present(page: initialWaveFxPage)
        } else {
            dsp.hide()
        }
    }

    func render(_ event: DeviceEvent) {
        dispatchPrecondition(condition: .onQueue(.main))
        switch event {
        case let .connected(connectedModel, displayName, info, config):
            model = connectedModel
            pendingWrites = 0
            deviceName.stringValue = displayName
            firmware.stringValue = "Firmware: \(info.firmwareVersion)"
            serialNumber = info.serial
            updateSerial()
            outputVolume.slider.minValue = model == .wave3Mk2 ? -60 : -128
            dial.removeAllItems()
            dial.addItems(withTitles: model == .wave3
                ? ["Input gain", "Output volume", "Mic/PC monitor mix"]
                : ["Input gain", "Output volume"])
            dialGroup.isHidden = model == .wave3Mk2
            dspGroup.isHidden = model != .wave3Mk2
            if model != .wave3Mk2 {
                dsp.hide()
            }
            setConnected(true)
            status.isHidden = true
            render(config)
            if model == .wave3Mk2, let initialWaveFxPage {
                dsp.present(page: initialWaveFxPage)
            }
            fitWindowToContent()
        case let .configChanged(config):
            pendingWrites = max(0, pendingWrites - 1)
            render(config)
        case let .refreshed(config):
            render(config)
        case let .disconnected(message):
            pendingWrites = 0
            cancelPendingSliders()
            serialNumber = ""
            serialVisible = false
            updateSerial()
            dsp.hide()
            dsp.cancelPendingCommits()
            dspGroup.isHidden = true
            setConnected(false)
            showStatus(message, color: .systemRed)
        case let .operationFailed(message):
            pendingWrites = max(0, pendingWrites - 1)
            showStatus("Setting failed: \(message)", color: .systemRed)
        }
    }

    private func render(_ config: DeviceConfig) {
        guard pendingWrites == 0 else { return }
        syncing = true
        gain.render(config.gainDb)
        muteSwitch.state = config.muted ? .on : .off
        clipguardSwitch.state = config.clipguard ? .on : .off
        lowcutSwitch.state = config.lowcut ? .on : .off
        outputVolume.render(config.outputVolumeDb)
        monitorMix.render(config.monitorMixPercent)
        if model != .wave3Mk2 {
            let selection: Int?
            switch (model, config.dialMode) {
            case (_, .inputGain?): selection = 0
            case (_, .outputVolume?): selection = 1
            case (.wave3, .monitorMix?): selection = 2
            default: selection = nil
            }
            if let selection {
                dial.selectItem(at: selection)
            }
            dial.isEnabled = connected && selection != nil
            if selection == nil {
                showStatus("Unknown dial function", color: .systemOrange)
            }
        }
        syncing = false
        if model == .wave3Mk2 {
            dsp.render(config)
        }
    }

    private func setConnected(_ connected: Bool) {
        self.connected = connected
        for control in [gain.slider, muteSwitch, clipguardSwitch, lowcutSwitch, outputVolume.slider, monitorMix.slider, dial, dspButton, serialButton] {
            control.isEnabled = connected
        }
        dsp.setEnabled(connected && model == .wave3Mk2)
    }

    var hasPendingWrites: Bool {
        pendingWrites > 0
    }

    func applySetting(_ setting: Setting) {
        guard connected, !syncing, let worker else { return }
        pendingWrites += 1
        worker.apply(setting)
    }

    private func cancelPendingSliders() {
        gain.cancelPendingCommit()
        outputVolume.cancelPendingCommit()
        monitorMix.cancelPendingCommit()
    }

    private func showStatus(_ message: String, color: NSColor) {
        status.stringValue = message
        status.textColor = color
        status.isHidden = false
        fitWindowToContent()
    }

    private func updateSerial() {
        let length = serialNumber.isEmpty ? 12 : serialNumber.count
        serial.stringValue = "Serial: \(serialVisible && !serialNumber.isEmpty ? serialNumber : String(repeating: "*", count: length))"
        let label = serialVisible ? "Hide serial number" : "Show serial number"
        serialButton.image = NSImage(systemSymbolName: serialVisible ? "eye.slash" : "eye", accessibilityDescription: label)
        serialButton.toolTip = label
        serialButton.setAccessibilityLabel(label)
    }

    private func switchRow(_ title: String, control: NSSwitch, identifier: String) -> NSView {
        control.target = self
        control.action = #selector(changeSwitch(_:))
        control.setAccessibilityLabel(title)
        control.setAccessibilityIdentifier(identifier)
        return valueRow(title, control: control)
    }

    @objc private func presentDsp() {
        dsp.present(page: 0)
    }

    @objc private func toggleSerial() {
        serialVisible.toggle()
        updateSerial()
    }

    @objc private func changeSwitch(_ sender: NSSwitch) {
        guard !syncing else { return }
        let enabled = sender.state == .on
        if sender === muteSwitch {
            applySetting(.mute(enabled))
        } else if sender === clipguardSwitch {
            applySetting(.clipguard(enabled))
        } else if sender === lowcutSwitch {
            applySetting(.lowcut(enabled))
        }
    }

    @objc private func changeDial() {
        guard !syncing, dial.isEnabled else { return }
        switch dial.indexOfSelectedItem {
        case 0: applySetting(.dialMode(.inputGain))
        case 1: applySetting(.dialMode(.outputVolume))
        case 2 where model == .wave3: applySetting(.dialMode(.monitorMix))
        default: break
        }
    }
}


extension MainViewController {
    func demoSteps(for model: DeviceModel) -> [DemoStep] {
        var steps = [
            DemoStep(name: "gain write", action: {
                self.gain.slider.doubleValue = 13
                self.trigger(self.gain.slider)
            }, matches: { $0.gainDb == 13 }),
            DemoStep(name: "mute write", action: {
                self.muteSwitch.state = .on
                self.trigger(self.muteSwitch)
            }, matches: { $0.muted }),
            DemoStep(name: "Clipguard write", action: {
                self.clipguardSwitch.state = .off
                self.trigger(self.clipguardSwitch)
            }, matches: { !$0.clipguard }),
            DemoStep(name: "Lowcut write", action: {
                self.lowcutSwitch.state = .off
                self.trigger(self.lowcutSwitch)
            }, matches: { !$0.lowcut }),
            DemoStep(name: "headphone-volume write", action: {
                self.outputVolume.slider.doubleValue = -20
                self.trigger(self.outputVolume.slider)
            }, matches: { $0.outputVolumeDb == -20 }),
            DemoStep(name: "monitor-mix write", action: {
                self.monitorMix.slider.doubleValue = 25
                self.trigger(self.monitorMix.slider)
            }, matches: { $0.monitorMixPercent == 25 }),
        ]
        if model == .wave3 {
            steps.append(DemoStep(name: "dial-function write", action: {
                self.dial.selectItem(at: 1)
                self.trigger(self.dial)
            }, matches: { $0.dialMode == .outputVolume }))
        } else if model == .wave3Mk2 {
            dsp.present(page: initialWaveFxPage ?? 0)
            steps.append(contentsOf: dsp.demoSteps())
        }
        return steps
    }

    private func trigger(_ control: NSControl) {
        _ = control.sendAction(control.action, to: control.target)
    }
}
