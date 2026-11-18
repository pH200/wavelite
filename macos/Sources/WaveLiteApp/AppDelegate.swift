import AppKit
import Foundation
import WaveLiteKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let mainViewController = MainViewController()
    private var window: NSWindow?
    private var screenshotPath: String?
    private var screenshotScheduled = false
    private var demoTest: DemoTestDriver?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: mainWindowContentWidth, height: 1),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "WaveLite"
        window.contentViewController = mainViewController
        mainViewController.fitWindowToContent()
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
        self.window = window

        let environment = ProcessInfo.processInfo.environment
        let demoModel: DeviceModel?
        switch environment["WAVELITE_FAKE_MODEL"] {
        case "wave3-mk1": demoModel = .wave3
        case "wave3-mk2": demoModel = .wave3Mk2
        default: demoModel = nil
        }
        let delay = environment["WAVELITE_FAKE_DELAY_MS"].flatMap(UInt64.init) ?? 0
        screenshotPath = environment["WAVELITE_SCREENSHOT"]
        switch environment["WAVELITE_FAKE_WAVE_FX"] {
        case "dynamics": mainViewController.initialWaveFxPage = 0
        case "eq": mainViewController.initialWaveFxPage = 1
        default: mainViewController.initialWaveFxPage = nil
        }
        if environment["WAVELITE_DEMO_SELF_TEST"] != nil {
            guard demoModel != nil else {
                fputs("The AppKit demo self-test requires WAVELITE_FAKE_MODEL\n", stderr)
                exit(1)
            }
            let driver = DemoTestDriver(delayedIO: delay > 0) { [weak self] outcome in
                self?.finishDemoTest(outcome)
            }
            demoTest = driver
            driver.start()
        }
        do {
            let core = try CoreController(demoModel: demoModel, delayMilliseconds: delay)
            let worker = DeviceWorker(core: core) { [weak self] event in
                self?.render(event)
            }
            mainViewController.attach(worker: worker)
        } catch {
            render(.disconnected(error.localizedDescription))
        }
    }

    private func render(_ event: DeviceEvent) {
        mainViewController.render(event)
        if let demoTest {
            demoTest.handle(event, in: mainViewController)
        } else if case .connected = event, screenshotPath != nil, !screenshotScheduled {
            screenshotScheduled = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                self.captureScreenshot()
            }
        }
    }

    private func finishDemoTest(_ outcome: DemoTestOutcome) {
        switch outcome {
        case let .passed(settings, ticks):
            print("AppKit demo self-test passed for \(settings) settings: \(ticks) UI timer ticks")
            if screenshotPath == nil {
                NSApplication.shared.terminate(nil)
            } else {
                captureScreenshot()
            }
        case let .failed(message):
            fputs("AppKit demo self-test failed: \(message)\n", stderr)
            exit(1)
        }
    }

    private func captureScreenshot() {
        guard let path = screenshotPath else { return }
        let view = mainViewController.captureView()
        view.layoutSubtreeIfNeeded()
        guard let image = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            fputs("Could not create the AppKit screenshot\n", stderr)
            exit(1)
        }
        view.cacheDisplay(in: view.bounds, to: image)
        guard let data = image.representation(using: .png, properties: [:]) else {
            fputs("Could not encode the AppKit screenshot\n", stderr)
            exit(1)
        }
        do {
            let url = URL(fileURLWithPath: path)
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: url)
            print("Saved screenshot to \(path)")
            NSApplication.shared.terminate(nil)
        } catch {
            fputs("Could not save screenshot to \(path): \(error.localizedDescription)\n", stderr)
            exit(1)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        demoTest?.stop()
        mainViewController.stop()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}
