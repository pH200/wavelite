// swift-tools-version: 6.0
import Foundation
import PackageDescription

let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let repositoryRoot = packageRoot.deletingLastPathComponent()
let coreLibraryDirectory = ProcessInfo.processInfo.environment["WAVELITE_CORE_LIBRARY_DIR"]
    ?? repositoryRoot.appendingPathComponent("target/debug").path

let package = Package(
    name: "WaveLite",
    platforms: [.macOS(.v12)],
    products: [
        .library(name: "WaveLiteKit", targets: ["WaveLiteKit"]),
        .executable(name: "WaveLite", targets: ["WaveLiteApp"]),
    ],
    targets: [
        .target(
            name: "WaveLiteCoreFFI",
            path: "Sources/WaveLiteCoreFFI",
            publicHeadersPath: "include",
            linkerSettings: [
                .unsafeFlags(["-L\(coreLibraryDirectory)"]),
                .linkedLibrary("wavelite"),
                .linkedLibrary("iconv"),
                .linkedLibrary("resolv"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("IOKit"),
                .linkedFramework("Security"),
            ]
        ),
        .target(
            name: "WaveLiteKit",
            dependencies: ["WaveLiteCoreFFI"],
            path: "Sources/WaveLiteKit",
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
        .executableTarget(
            name: "WaveLiteApp",
            dependencies: ["WaveLiteKit"],
            path: "Sources/WaveLiteApp",
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
        .testTarget(
            name: "WaveLiteKitTests",
            dependencies: ["WaveLiteKit"],
            path: "Tests/WaveLiteKitTests",
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
    ]
)
