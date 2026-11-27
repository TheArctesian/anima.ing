// swift-tools-version:5.3
// The iOS side of the haptics plugin (mobile plan M4-4).

import PackageDescription

let package = Package(
  name: "tauri-plugin-haptics",
  platforms: [.iOS(.v13)],
  products: [
    .library(name: "tauri-plugin-haptics", type: .static, targets: ["tauri-plugin-haptics"])
  ],
  dependencies: [.package(name: "Tauri", path: "../.tauri/tauri-api")],
  targets: [
    .target(name: "tauri-plugin-haptics", dependencies: [.byName(name: "Tauri")], path: "Sources")
  ]
)
