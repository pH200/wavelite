import XCTest
@testable import WaveLiteKit

final class CoreControllerTests: XCTestCase {
    func testSwiftSettingRoundTripsThroughRustFakeDevice() throws {
        let core = try CoreController(demoModel: .wave3)
        var events: [DeviceEvent] = []
        core.eventHandler = { events.append($0) }

        XCTAssertTrue(core.executeNext())
        core.apply(.gain(12.5))
        core.eventHandler = nil

        XCTAssertEqual(events.count, 2)
        guard case .connected = events[0] else {
            return XCTFail("Expected connection before the write")
        }
        guard case let .configChanged(config) = events[1] else {
            return XCTFail("Expected a changed configuration")
        }
        XCTAssertEqual(config.gainDb, 12.5)
    }

    func testVoiceTuneStrengthRoundTripsThroughRustFakeDevice() throws {
        let core = try CoreController(demoModel: .wave3Mk2)
        var events: [DeviceEvent] = []
        core.eventHandler = { events.append($0) }

        XCTAssertTrue(core.executeNext())
        core.apply(.voiceTuneStrength(65))
        core.eventHandler = nil

        guard case let .configChanged(config) = events.last else {
            return XCTFail("Expected a changed configuration")
        }
        XCTAssertEqual(config.voiceTuneStrengthPercent, 65)
    }

    func testTypedEqAndDynamicsRoundTripThroughRustFakeDevice() throws {
        let core = try CoreController(demoModel: .wave3Mk2)
        var events: [DeviceEvent] = []
        core.eventHandler = { events.append($0) }

        XCTAssertTrue(core.executeNext())
        core.apply(.eqBand(
            index: 2,
            update: EqBandUpdate(enabled: true, kind: .bell, gainDb: 3)
        ))
        core.apply(.dynamics(DynamicsUpdate(
            compressorThresholdDb: -20,
            expanderReleaseMs: 200
        )))
        core.eventHandler = nil

        XCTAssertEqual(events.count, 3)
        guard case let .configChanged(eqConfig) = events[1],
              case let .configChanged(dynamicsConfig) = events[2]
        else {
            return XCTFail("Expected changed configurations")
        }
        let band = try XCTUnwrap(eqConfig.eq?.bands[1])
        XCTAssertEqual(band.kind, .bell)
        XCTAssertEqual(band.gainDb, 3)
        XCTAssertEqual(dynamicsConfig.compressorSettings?.thresholdDb, -20)
        XCTAssertEqual(dynamicsConfig.expanderSettings?.releaseMs, 200)
    }
}
