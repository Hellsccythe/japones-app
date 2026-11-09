package com.japonesapp.desktop

import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  /** Last safe areas in CSS px, as JSON for client/js/insets.js. */
  @Volatile private var insetsJson = "{}"

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // Edge to edge, the WebView draws under the status bar, the notch and the
  // gesture bar but reports 0 for env(safe-area-inset-*): hand the page the
  // real insets, read at start-up and pushed again whenever they change.
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(object {
      @JavascriptInterface fun get(): String = insetsJson
    }, "AndroidInsets")
    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, insets ->
      val i = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val d = view.resources.displayMetrics.density
      insetsJson = """{"top":${i.top / d},"right":${i.right / d},"bottom":${i.bottom / d},"left":${i.left / d}}"""
      (view as WebView).evaluateJavascript("window.__setInsets && window.__setInsets('$insetsJson')", null)
      insets
    }
    ViewCompat.requestApplyInsets(webView)
  }
}
