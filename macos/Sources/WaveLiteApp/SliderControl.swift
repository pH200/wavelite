import AppKit

final class SliderControl: NSStackView {
    let slider: NSSlider
    private let valueLabel = NSTextField(labelWithString: "")
    private let digits: Int
    private let unit: String
    private var pendingWork: DispatchWorkItem?
    private var syncing = false
    private(set) var hasPendingCommit = false
    var onCommit: ((Double) -> Void)?

    init(title: String, minimum: Double, maximum: Double, digits: Int, unit: String) {
        self.digits = digits
        self.unit = unit
        slider = NSSlider(value: minimum, minValue: minimum, maxValue: maximum, target: nil, action: nil)
        super.init(frame: .zero)

        orientation = .vertical
        spacing = 6
        edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 6, right: 12)
        slider.target = self
        slider.action = #selector(valueChanged)
        slider.isContinuous = true
        slider.setAccessibilityLabel(title)

        let titleLabel = NSTextField(labelWithString: title)
        valueLabel.alignment = .right
        let heading = NSStackView(views: [titleLabel, valueLabel])
        heading.orientation = .horizontal
        heading.distribution = .equalSpacing
        addArrangedSubview(heading)
        addArrangedSubview(slider)
        heading.widthAnchor.constraint(equalTo: widthAnchor, constant: -24).isActive = true
        slider.widthAnchor.constraint(equalTo: widthAnchor, constant: -24).isActive = true
        updateValue(minimum)
    }

    required init?(coder: NSCoder) {
        fatalError("SliderControl does not support Interface Builder")
    }

    func render(_ value: Double) {
        guard !hasPendingCommit else { return }
        syncing = true
        slider.doubleValue = value
        syncing = false
        updateValue(value)
    }

    func cancelPendingCommit() {
        pendingWork?.cancel()
        pendingWork = nil
        hasPendingCommit = false
    }

    private func updateValue(_ value: Double) {
        let separator = unit == "%" ? "" : " "
        valueLabel.stringValue = String(format: "%.\(digits)f%@%@", value, separator, unit)
    }

    @objc private func valueChanged() {
        let value = slider.doubleValue
        updateValue(value)
        guard !syncing, slider.isEnabled else { return }
        pendingWork?.cancel()
        hasPendingCommit = true
        let work = DispatchWorkItem { [weak self] in
            guard let self else { return }
            self.pendingWork = nil
            self.hasPendingCommit = false
            self.onCommit?(self.slider.doubleValue)
        }
        pendingWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: work)
    }
}
