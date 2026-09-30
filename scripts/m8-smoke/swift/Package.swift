// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TtcM8Smoke",
    products: [.executable(name: "TtcM8Smoke", targets: ["TtcM8Smoke"])],
    targets: [
        .executableTarget(name: "TtcM8Smoke"),
        .testTarget(name: "TtcM8SmokeTests", dependencies: ["TtcM8Smoke"]),
    ]
)
