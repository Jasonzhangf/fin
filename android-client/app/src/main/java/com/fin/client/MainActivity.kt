package com.fin.client

import android.annotation.SuppressLint
import android.os.Bundle
import android.webkit.WebChromeClient
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import java.io.File
import com.fin.client.bridge.MobileBridge
import com.fin.client.storage.WsProfileStore

class MainActivity : ComponentActivity() {
    private lateinit var webView: WebView

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        webView = WebView(this)
        val profileStore = WsProfileStore(this)
        val bridge = MobileBridge(this, profileStore)

        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            cacheMode = WebSettings.LOAD_DEFAULT
            mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
        }
        webView.webViewClient = WebViewClient()
        webView.webChromeClient = WebChromeClient()
        webView.addJavascriptInterface(bridge, "FinMobileBridge")

        copyUpdateManifestIfPresent()
        setContentView(webView)
        // MVP: 直接复用现有 WebUI（本地/远程地址可在 assets 引导页里切换）
        webView.loadUrl("file:///android_asset/mobile-shell.html")
    }



    private fun copyUpdateManifestIfPresent() {
        runCatching {
            val source = File("/sdcard/Download/latest.json")
            val targetDir = File(filesDir.parentFile, "app_update_dist")
            if (!targetDir.exists()) targetDir.mkdirs()
            val target = File(targetDir, "latest.json")
            if (source.exists()) source.copyTo(target, overwrite = true)
        }
    }

    override fun onDestroy() {
        webView.removeJavascriptInterface("FinMobileBridge")
        webView.destroy()
        super.onDestroy()
    }
}
