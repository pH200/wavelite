import Foundation

public protocol DeviceCore: AnyObject {
    var eventHandler: ((DeviceEvent) -> Void)? { get set }
    func nextDelay() -> TimeInterval
    func executeNext() -> Bool
    func apply(_ setting: Setting)
}

public enum CoreError: LocalizedError {
    case unsupportedDemoModel

    public var errorDescription: String? {
        "The Wave:1 does not have a fake-device demo"
    }
}

private final class EventContext: WaveLiteEventCallback, @unchecked Sendable {
    private let lock = NSLock()
    private var handler: ((DeviceEvent) -> Void)?

    func setHandler(_ handler: ((DeviceEvent) -> Void)?) {
        lock.lock()
        self.handler = handler
        lock.unlock()
    }

    func currentHandler() -> ((DeviceEvent) -> Void)? {
        lock.lock()
        defer { lock.unlock() }
        return handler
    }

    func onEvent(event: DeviceEvent) {
        currentHandler()?(event)
    }
}

public final class CoreController: DeviceCore {
    private let controller: WaveLiteController
    private let context: EventContext

    public var eventHandler: ((DeviceEvent) -> Void)? {
        get { context.currentHandler() }
        set { context.setHandler(newValue) }
    }

    public init(demoModel: DeviceModel? = nil, delayMilliseconds: UInt64 = 0) throws {
        let context = EventContext()
        self.context = context
        switch demoModel {
        case .none:
            controller = WaveLiteController.usb(callback: context)
        case .wave3:
            controller = WaveLiteController.demo(
                model: .wave3,
                delayMs: delayMilliseconds,
                callback: context
            )
        case .wave3Mk2:
            controller = WaveLiteController.demo(
                model: .wave3Mk2,
                delayMs: delayMilliseconds,
                callback: context
            )
        case .wave1:
            throw CoreError.unsupportedDemoModel
        }
    }

    deinit {
        eventHandler = nil
    }

    public func nextDelay() -> TimeInterval {
        Double(controller.nextDelayMs()) / 1_000
    }

    public func executeNext() -> Bool {
        controller.executeNext()
    }

    public func apply(_ setting: Setting) {
        controller.apply(setting: setting)
    }
}
