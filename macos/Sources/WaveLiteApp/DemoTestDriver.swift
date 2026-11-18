import AppKit
import WaveLiteKit

struct DemoStep {
    let name: String
    let writeCount: Int
    let action: () -> Void
    let matches: (DeviceConfig) -> Bool

    init(name: String, writeCount: Int = 1, action: @escaping () -> Void, matches: @escaping (DeviceConfig) -> Bool) {
        self.name = name
        self.writeCount = writeCount
        self.action = action
        self.matches = matches
    }
}

enum DemoTestOutcome {
    case passed(settings: Int, ticks: Int)
    case failed(String)
}

final class DemoTestDriver {
    private let delayedIO: Bool
    private let completion: (DemoTestOutcome) -> Void
    private var timer: Timer?
    private var steps: [DemoStep]?
    private var stepIndex = 0
    private var stepMark = 0
    private var receivedWrites = 0
    private weak var viewController: MainViewController?
    private var ticks = 0
    private var finished = false

    init(delayedIO: Bool, completion: @escaping (DemoTestOutcome) -> Void) {
        self.delayedIO = delayedIO
        self.completion = completion
    }

    func start() {
        dispatchPrecondition(condition: .onQueue(.main))
        let timer = Timer(timeInterval: 0.025, repeats: true) { [weak self] _ in
            self?.tick()
        }
        self.timer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        finished = true
    }

    func handle(_ event: DeviceEvent, in viewController: MainViewController) {
        guard !finished else { return }
        switch event {
        case let .connected(model, _, _, _):
            guard ticks >= 4 || !delayedIO else {
                finish(.failed("UI was blocked during delayed connection: only \(ticks) timer ticks"))
                return
            }
            let steps = viewController.demoSteps(for: model)
            guard !steps.isEmpty else {
                finish(.failed("No demo steps are available for \(model)"))
                return
            }
            self.steps = steps
            self.viewController = viewController
            startStep(0)
        case let .configChanged(config):
            guard let steps, stepIndex < steps.count else { return }
            let step = steps[stepIndex]
            receivedWrites += 1
            guard receivedWrites <= step.writeCount else {
                finish(.failed("Too many readbacks after \(step.name)"))
                return
            }
            guard receivedWrites == step.writeCount else { return }
            guard step.matches(config) else {
                finish(.failed("Unexpected readback after \(step.name)"))
                return
            }
            let elapsed = ticks - stepMark
            guard elapsed >= 3 || !delayedIO else {
                finish(.failed("UI was blocked during \(step.name): only \(elapsed) timer ticks"))
                return
            }
            startStep(stepIndex + 1)
        case let .disconnected(message):
            finish(.failed("Device disconnected during demo test: \(message)"))
        case let .operationFailed(message):
            finish(.failed("Device operation failed during demo test: \(message)"))
        case .refreshed:
            break
        }
    }

    private func startStep(_ index: Int) {
        guard let steps else { return }
        guard index < steps.count else {
            viewController?.finishDemoTest()
            finish(.passed(settings: index, ticks: ticks))
            return
        }
        stepIndex = index
        stepMark = ticks
        receivedWrites = 0
        print("Demo step \(index + 1)/\(steps.count): \(steps[index].name)")
        fflush(stdout)
        steps[index].action()
    }

    private func tick() {
        guard !finished else { return }
        ticks += 1
        if ticks >= 4_800 {
            finish(.failed("Timed out waiting for device I/O"))
        }
    }

    private func finish(_ outcome: DemoTestOutcome) {
        guard !finished else { return }
        timer?.invalidate()
        timer = nil
        finished = true
        completion(outcome)
    }
}
