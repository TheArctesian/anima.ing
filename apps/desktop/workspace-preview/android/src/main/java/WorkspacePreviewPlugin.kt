package ing.anima.workspacepreview

import android.app.Activity
import android.app.Dialog
import android.graphics.Color
import android.net.Uri
import android.net.http.SslError
import android.view.ViewGroup
import android.webkit.*
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import org.json.JSONObject

@InvokeArg
internal class PreviewArgs { lateinit var id: String; lateinit var action: String; lateinit var token: String }
@InvokeArg
internal class CloseArgs { lateinit var id: String }

@TauriPlugin
class WorkspacePreviewPlugin(private val host: Activity) : Plugin(host) {
    private var shown: Dialog? = null
    private var owner: String? = null

    @Command
    fun open(invoke: Invoke) {
        val args = invoke.parseArgs(PreviewArgs::class.java)
        val destination = Uri.parse(args.action)
        if (destination.scheme != "https" || destination.host.isNullOrEmpty() || destination.userInfo != null || destination.path != "/.anima/handoff" || destination.query != null || destination.fragment != null || !args.token.matches(Regex("^[A-Za-z0-9_-]{43}$"))) { invoke.reject("invalid"); return }
        host.runOnUiThread {
            if (shown != null || host.isFinishing || host.isDestroyed) { invoke.reject("busy"); return@runOnUiThread }
            val dialog = Dialog(host, android.R.style.Theme_Material_Light_NoActionBar)
            val layout = LinearLayout(host).apply { orientation = LinearLayout.VERTICAL }
            val bar = LinearLayout(host)
            val close = Button(host).apply { text = "Done"; setOnClickListener { dialog.dismiss() } }
            val title = TextView(host).apply { text = "Workspace preview"; setTextColor(Color.BLACK) }
            val web = WebView(host)
            bar.addView(close); bar.addView(title, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            bar.addView(Button(host).apply { text = "Reload"; setOnClickListener { if (web.url != args.action) web.reload() } })
            layout.addView(bar)
            layout.addView(web, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
            web.settings.javaScriptEnabled = true
            web.settings.domStorageEnabled = true
            web.settings.allowFileAccess = false
            web.settings.allowContentAccess = false
            web.settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            web.settings.setSupportMultipleWindows(true)
            web.webChromeClient = WebChromeClient()
            CookieManager.getInstance().setAcceptThirdPartyCookies(web, false)
            val expected = origin(destination)
            val launch = "$expected/.anima/connect"
            var token: String? = args.token
            args.token = ""
            web.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = request.url.scheme != "https" || origin(request.url) != expected
                override fun onReceivedSslError(view: WebView, handler: SslErrorHandler, error: SslError) { handler.cancel(); title.text = "Preview certificate could not be verified" }
                override fun onPageFinished(view: WebView, url: String) {
                    if (url != launch || view.url != launch) return
                    val secret = token ?: return
                    token = null
                    val script = "(()=>{if(location.origin!==${JSONObject.quote(expected)}||location.pathname!=='/.anima/connect')return;fetch('/.anima/handoff',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/x-www-form-urlencoded'},body:'token='+${JSONObject.quote(secret)}}).then(r=>{if(r.ok)location.replace('/');else document.body.textContent='Preview expired. Prepare it again in Code.';}).catch(()=>{document.body.textContent='Preview could not connect.';});})();"
                    view.evaluateJavascript(script, null)
                }
            }
            dialog.setContentView(layout)
            shown = dialog; owner = args.id
            dialog.setOnDismissListener {
                token = null; web.stopLoading(); web.destroy()
                if (shown === dialog) { shown = null; owner = null }
            }
            dialog.show()
            web.loadUrl(launch)
            invoke.resolve()
        }
    }
    @Command
    fun close(invoke: Invoke) {
        val args = invoke.parseArgs(CloseArgs::class.java)
        host.runOnUiThread { if (owner == args.id) shown?.dismiss(); invoke.resolve() }
    }
    private fun origin(url: Uri): String = "https://${url.host}" + if (url.port == -1) "" else ":${url.port}"
}
