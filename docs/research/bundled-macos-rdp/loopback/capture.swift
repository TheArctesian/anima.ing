import AppKit
import CoreGraphics
import ScreenCaptureKit

@main
struct CaptureProbe {
    @MainActor static func main() async throws {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        guard CGPreflightScreenCaptureAccess() else {
            throw NSError(domain: "AnimaRDPProbe", code: 1)
        }
        let window = NSWindow(contentRect: NSRect(x: 50, y: 50, width: 64, height: 64),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.title = "Anima RDP synthetic capture"
        window.backgroundColor = NSColor(srgbRed: 200.0 / 255, green: 30.0 / 255, blue: 90.0 / 255, alpha: 1)
        window.isOpaque = true
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }

        var target: SCWindow? = nil
        for _ in 0..<20 {
            let own = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
            target = own.windows.first { $0.windowID == CGWindowID(window.windowNumber) }
            if target != nil { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        guard let target else { throw NSError(domain: "AnimaRDPProbe", code: 2) }
        let config = SCStreamConfiguration()
        config.width = 64
        config.height = 64
        config.showsCursor = false
        config.ignoreShadowsSingleWindow = true
        let image = try await SCScreenshotManager.captureImage(
            contentFilter: SCContentFilter(desktopIndependentWindow: target),
            configuration: config
        )
        let colourSpace = CGColorSpace(name: CGColorSpace.sRGB)!
        let ctx = CGContext(data: nil, width: 64, height: 64, bitsPerComponent: 8,
                            bytesPerRow: 64 * 4, space: colourSpace,
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: 64, height: 64))
        let data = ctx.data!.assumingMemoryBound(to: UInt8.self)
        let match = Array(0..<3).allSatisfy { index in
            abs(Int(data[(32 * 64 + 32) * 4 + index]) - [200, 30, 90][index]) <= 3
        }
        guard match else { throw NSError(domain: "AnimaRDPProbe", code: 3) }
        var bgra = Data(capacity: 32 * 32 * 4)
        for y in 16..<48 {
            for x in 16..<48 {
                let offset = (y * 64 + x) * 4
                bgra.append(contentsOf: [data[offset + 2], data[offset + 1], data[offset], data[offset + 3]])
            }
        }
        try bgra.write(to: URL(fileURLWithPath: "synthetic-window.bgra"))
        print("ScreenCaptureKit captured the expected colour from the probe's own 64×64 test window.")
    }
}
