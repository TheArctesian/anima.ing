import SwiftRs
import Tauri
import UIKit
import WebKit

struct PreviewArgs: Decodable { let id: String; let action: String; let token: String }
struct CloseArgs: Decodable { let id: String }

final class WorkspacePreviewPlugin: Plugin, UIAdaptivePresentationControllerDelegate {
  private var shown: UINavigationController?
  private var owner: String?

  @objc public func open(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PreviewArgs.self)
    guard let url = URL(string: args.action), url.scheme == "https", url.path == "/.anima/handoff",
      url.user == nil, url.password == nil, url.query == nil, url.fragment == nil,
      args.token.range(of: "^[A-Za-z0-9_-]{43}$", options: .regularExpression) != nil else { invoke.reject("invalid"); return }
    DispatchQueue.main.async {
      guard self.shown == nil, let presenter = self.manager.viewController, presenter.presentedViewController == nil else { invoke.reject("busy"); return }
      let page = PreviewPage(url: url, token: args.token)
      let navigation = UINavigationController(rootViewController: page)
      page.dismissPreview = { [weak self] in self?.finish() }
      navigation.modalPresentationStyle = .fullScreen
      self.shown = navigation; self.owner = args.id
      presenter.present(navigation, animated: true) { invoke.resolve() }
      navigation.presentationController?.delegate = self
    }
  }
  @objc public func close(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(CloseArgs.self)
    DispatchQueue.main.async { if self.owner == args.id { self.finish() }; invoke.resolve() }
  }
  private func finish() {
    (shown?.viewControllers.first as? PreviewPage)?.stop()
    shown?.dismiss(animated: true); shown = nil; owner = nil
  }
  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { finish() }
}

private final class PreviewPage: UIViewController, WKNavigationDelegate {
  private let destination: URL
  private var token: String?
  private var browser: WKWebView?
  var dismissPreview: (() -> Void)?
  init(url: URL, token: String) { destination = url; self.token = token; super.init(nibName: nil, bundle: nil) }
  required init?(coder: NSCoder) { fatalError("not supported") }
  override func viewDidLoad() {
    super.viewDidLoad(); title = "Workspace preview"
    navigationItem.leftBarButtonItem = UIBarButtonItem(barButtonSystemItem: .done, target: self, action: #selector(done))
    navigationItem.rightBarButtonItem = UIBarButtonItem(barButtonSystemItem: .refresh, target: self, action: #selector(refresh))
    let configuration = WKWebViewConfiguration()
    configuration.websiteDataStore = .nonPersistent()
    let web = WKWebView(frame: .zero, configuration: configuration)
    web.navigationDelegate = self; web.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(web)
    NSLayoutConstraint.activate([web.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor), web.bottomAnchor.constraint(equalTo: view.bottomAnchor), web.leadingAnchor.constraint(equalTo: view.leadingAnchor), web.trailingAnchor.constraint(equalTo: view.trailingAnchor)])
    browser = web
    var request = URLRequest(url: destination); request.httpMethod = "POST"
    request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
    request.setValue(origin(destination), forHTTPHeaderField: "Origin")
    request.httpBody = ("token=" + (token ?? "")).data(using: .utf8); token = nil
    web.load(request)
  }
  @objc private func done() { dismissPreview?() }
  @objc private func refresh() { if let url = browser?.url, url.path != "/.anima/handoff" { browser?.reload() } }
  func stop() { browser?.stopLoading(); browser?.navigationDelegate = nil; browser?.removeFromSuperview(); browser = nil; token = nil }
  func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
    guard let url = action.request.url, url.scheme == "https", origin(url) == origin(destination) else { decisionHandler(.cancel); return }
    decisionHandler(.allow)
  }
  private func origin(_ url: URL) -> String { "https://" + (url.host ?? "") + (url.port.map { ":\($0)" } ?? "") }
}

@_cdecl("init_plugin_workspace_preview")
public func initPlugin() -> Plugin { WorkspacePreviewPlugin() }
