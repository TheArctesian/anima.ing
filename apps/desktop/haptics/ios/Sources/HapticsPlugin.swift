// The haptic feedback of the iPhone app (mobile plan M4-4, MOT-050): the patterns of UIKit, named for what they mean.
// iOS plays nothing when System Haptics is off in Settings, so the app has no setting of its own.

import SwiftRs
import Tauri
import UIKit

enum Feel: String, Decodable {
  case selection, success, error
}

struct PlayArgs: Decodable {
  let feel: Feel
}

class HapticsPlugin: Plugin {
  // UIKit's generators live on the main thread; `play` makes them there on its first call.
  private lazy var selection = UISelectionFeedbackGenerator()
  private lazy var notification = UINotificationFeedbackGenerator()

  /// Plays `feel`. A selection prepares the next one, so the tick of the next notch is not late (MOT-051).
  @objc public func play(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PlayArgs.self)
    DispatchQueue.main.async {
      switch args.feel {
      case .selection:
        self.selection.selectionChanged()
        self.selection.prepare()
      case .success:
        self.notification.notificationOccurred(.success)
      case .error:
        self.notification.notificationOccurred(.error)
      }
      invoke.resolve()
    }
  }
}

@_cdecl("init_plugin_haptics")
public func initPlugin() -> Plugin {
  return HapticsPlugin()
}
