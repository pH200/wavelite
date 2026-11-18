import Dispatch
import Foundation

public final class DeviceWorker {
    let queue = DispatchQueue(label: "dev.wavelite.WaveLite.device")
    private let queueKey = DispatchSpecificKey<UInt8>()
    private let core: DeviceCore
    private let eventHandler: (DeviceEvent) -> Void
    private var timer: DispatchSourceTimer?
    private var running = false

    public init(core: DeviceCore, eventHandler: @escaping (DeviceEvent) -> Void) {
        self.core = core
        self.eventHandler = eventHandler
        queue.setSpecific(key: queueKey, value: 1)
        core.eventHandler = { [weak self] event in
            self?.publish(event)
        }
    }

    public func start() {
        queue.async {
            guard !self.running else { return }
            self.running = true
            self.schedule()
        }
    }

    public func apply(_ setting: Setting) {
        queue.async {
            guard self.running else { return }
            self.core.apply(setting)
        }
    }

    public func stop() {
        if DispatchQueue.getSpecific(key: queueKey) == 1 {
            stopOnQueue()
        } else {
            queue.sync {
                self.stopOnQueue()
            }
        }
    }

    private func stopOnQueue() {
        running = false
        timer?.cancel()
        timer = nil
        core.eventHandler = nil
    }

    private func schedule() {
        guard running else { return }
        let delay = min(max(core.nextDelay(), 0), 86_400)
        let timer = DispatchSource.makeTimerSource(queue: queue)
        self.timer = timer
        timer.schedule(deadline: .now() + delay)
        timer.setEventHandler { [weak self] in
            guard let self, self.running else { return }
            self.timer = nil
            _ = self.core.executeNext()
            self.schedule()
        }
        timer.resume()
    }

    private func publish(_ event: DeviceEvent) {
        DispatchQueue.main.async {
            self.eventHandler(event)
        }
    }
}
