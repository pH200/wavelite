import XCTest
@testable import WaveLiteKit

final class DeviceWorkerTests: XCTestCase {
    func testCoreUsesSerialWorkerAndEventsArriveOnMainQueue() {
        let core = FakeCore()
        let connected = expectation(description: "connected")
        let changed = expectation(description: "changed")
        let worker = DeviceWorker(core: core) { event in
            XCTAssertTrue(Thread.isMainThread)
            switch event {
            case .connected: connected.fulfill()
            case .configChanged: changed.fulfill()
            default: break
            }
        }
        core.expectedQueue = worker.queue
        worker.start()
        worker.apply(.mute(true))
        wait(for: [connected, changed], timeout: 1)
        worker.stop()
    }

    func testDelayedCoreOperationDoesNotBlockMainQueue() {
        let core = FakeCore()
        core.executionDelay = 0.15
        let mainTick = expectation(description: "main tick")
        let connected = expectation(description: "connected")
        let worker = DeviceWorker(core: core) { event in
            if case .connected = event {
                connected.fulfill()
            }
        }
        core.expectedQueue = worker.queue
        worker.start()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            mainTick.fulfill()
        }
        wait(for: [mainTick, connected], timeout: 1, enforceOrder: true)
        worker.stop()
    }

    func testQueuedWritesRunBeforeScheduledPolling() {
        let core = FakeCore()
        core.delay = 0.05
        let connected = expectation(description: "connected")
        let worker = DeviceWorker(core: core) { event in
            if case .connected = event {
                XCTAssertEqual(core.operations, ["apply", "apply", "poll"])
                connected.fulfill()
            }
        }
        core.expectedQueue = worker.queue
        worker.start()
        worker.apply(.gain(12))
        worker.apply(.mute(true))
        wait(for: [connected], timeout: 1)
        worker.stop()
    }
}

private final class FakeCore: DeviceCore {
    var eventHandler: ((DeviceEvent) -> Void)?
    var expectedQueue: DispatchQueue?
    var delay = 0.0
    var executionDelay = 0.0
    private var hasConnected = false
    private(set) var operations: [String] = []

    func nextDelay() -> TimeInterval {
        assertQueue()
        return delay
    }

    func executeNext() -> Bool {
        assertQueue()
        if executionDelay > 0 {
            Thread.sleep(forTimeInterval: executionDelay)
        }
        operations.append("poll")
        if !hasConnected {
            hasConnected = true
            eventHandler?(Self.connected)
        }
        return true
    }

    func apply(_ setting: Setting) {
        assertQueue()
        operations.append("apply")
        eventHandler?(.configChanged(Self.changedConfig))
    }

    private func assertQueue() {
        XCTAssertFalse(Thread.isMainThread)
        if let expectedQueue {
            dispatchPrecondition(condition: .onQueue(expectedQueue))
        }
    }

    private static var connected: DeviceEvent {
        .connected(
            model: .wave3,
            displayName: "Elgato Wave:3 MK.1",
            info: DeviceInfo(
                apiVersion: "5.3",
                firmwareVersion: "1.2.4",
                serial: "W3MK1DEMO001",
                rawHex: "00"
            ),
            config: config
        )
    }

    private static var changedConfig: DeviceConfig {
        var config = config
        config.muted = true
        return config
    }

    private static var config: DeviceConfig {
        DeviceConfig(
            rawHex: "00",
            settingsHex: nil,
            headphonesHex: nil,
            monitorHex: nil,
            gainDb: 12,
            muted: false,
            clipguard: true,
            clipguardMode: nil,
            lowcut: true,
            expander: nil,
            voiceTune: nil,
            voiceTuneStrengthPercent: nil,
            compressor: nil,
            outputVolumeDb: -20,
            monitorMixPercent: 25,
            dialMode: .inputGain,
            eq: nil,
            compressorSettings: nil,
            expanderSettings: nil
        )
    }
}
