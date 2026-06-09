package com.fin.client

import android.annotation.SuppressLint
import android.os.Bundle
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.content.Context
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import android.text.InputType
import android.util.Log
import android.webkit.ConsoleMessage
import android.webkit.WebChromeClient
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.view.ViewTreeObserver
import androidx.activity.ComponentActivity
import java.io.File
import kotlin.concurrent.thread
import org.json.JSONObject
import com.fin.client.bridge.MobileBridge
import com.fin.client.storage.WsProfileStore

class MainActivity : ComponentActivity() {
    private lateinit var webView: WebView
    private lateinit var nativeInput: EditText
    private var nativeInputBar: LinearLayout? = null
    private var nativeComposer: LinearLayout? = null
    private var nativeActionRow: LinearLayout? = null
    private var nativeSendButton: TextView? = null
    private val tag = "fin-main"

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        webView = WebView(this)
        webView.isFocusable = true
        webView.isFocusableInTouchMode = true
        webView.requestFocus()
        val profileStore = WsProfileStore(this)
        val bridge = MobileBridge(
            this,
            profileStore,
            { theme -> applyNativeInputTheme(theme) },
            { mode -> applyNativeChromeMode(mode) },
        )
        bridge.attachWebView(webView)

        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            cacheMode = WebSettings.LOAD_DEFAULT
            mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
        }
        webView.webViewClient = WebViewClient()
        webView.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(consoleMessage: ConsoleMessage): Boolean {
                Log.i(tag, "wv-console ${consoleMessage.messageLevel()}: ${consoleMessage.message()} @${consoleMessage.sourceId()}:${consoleMessage.lineNumber()}")
                return super.onConsoleMessage(consoleMessage)
            }
        }

        webView.addJavascriptInterface(bridge, "FinMobileBridge")
        bridge.appendConnectionEvent("app.onCreate webview_init")

        copyUpdateManifestIfPresent()
        runUpdateSelfTestIfRequested(bridge)
        val root = FrameLayout(this)
        root.addView(webView, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        nativeInputBar = buildNativeInputBar(bridge)
        root.addView(nativeInputBar, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM))
        installNativeInputInsetSync(root, bridge)
        setContentView(root)
        // MVP: 直接复用现有 WebUI（本地/远程地址可在 assets 引导页里切换）
        webView.loadUrl("file:///android_asset/mobile-shell.html")

        // Restore state after page load
        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                bridge.appendConnectionEvent("app.page.finished url=${url ?: ""}")
                // Rehydrate state
                webView.evaluateJavascript(
                    "if(typeof CONFIG !== 'undefined') { CONFIG.load(); if(typeof restoreState === 'function') restoreState(); }",
                    null
                )
                intent?.getStringExtra("finAutoSend")?.takeIf { it.isNotBlank() }?.let { payload ->
                    val quoted = JSONObject.quote(payload)
                    webView.evaluateJavascript(
                        "setTimeout(function(){ if(typeof deviceE2eSend === 'function') deviceE2eSend($quoted); }, 2500);",
                        null
                    )
                    bridge.appendConnectionEvent("device_e2e_intent_autosend len=${payload.length}")
                }
            }
        }
    }


    private fun installNativeInputInsetSync(root: FrameLayout, bridge: MobileBridge) {
        val density = resources.displayMetrics.density
        var lastCssPx = -1
        fun sync() {
            val barHeight = nativeInputBar?.height ?: 0
            val visible = android.graphics.Rect()
            root.getWindowVisibleDisplayFrame(visible)
            val keyboardHeight = (root.rootView.height - visible.bottom).coerceAtLeast(0)
            val cssPx = ((barHeight + keyboardHeight) / density).toInt().coerceAtLeast(120)
            if (cssPx == lastCssPx) return
            lastCssPx = cssPx
            bridge.appendConnectionEvent("native_input_inset css_px=$cssPx bar_px=$barHeight keyboard_px=$keyboardHeight")
            webView.evaluateJavascript("if(typeof setNativeInputInset==='function') setNativeInputInset($cssPx);", null)
        }
        root.viewTreeObserver.addOnGlobalLayoutListener(object : ViewTreeObserver.OnGlobalLayoutListener {
            override fun onGlobalLayout() { sync() }
        })
        nativeInputBar?.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> sync() }
    }


    private fun buildNativeInputBar(bridge: MobileBridge): LinearLayout {
        val density = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * density).toInt()
        fun rounded(color: Int, radiusDp: Int, strokeDp: Int = 0, strokeColor: Int = Color.TRANSPARENT) =
            android.graphics.drawable.GradientDrawable().apply {
                shape = android.graphics.drawable.GradientDrawable.RECTANGLE
                cornerRadius = dp(radiusDp).toFloat()
                setColor(color)
                if (strokeDp > 0) setStroke(dp(strokeDp), strokeColor)
            }

        val outer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(8), dp(6), dp(8), dp(8))
            setBackgroundColor(Color.TRANSPARENT)
            ViewCompat.setOnApplyWindowInsetsListener(this) { view, insets ->
                val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
                val navBottom = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
                view.translationY = -imeBottom.toFloat()
                view.setPadding(dp(8), dp(6), dp(8), dp(8) + if (imeBottom > 0) 0 else navBottom)
                insets
            }
        }
        val composer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(12), dp(10), dp(12), dp(9))
            background = rounded(Color.rgb(21, 24, 31), 24, 1, Color.rgb(52, 58, 70))
            elevation = dp(10).toFloat()
        }
        nativeComposer = composer
        outer.addView(composer, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))

        nativeInput = EditText(this).apply {
            hint = "Ask anything"
            setHintTextColor(Color.rgb(137, 143, 155))
            setTextColor(Color.rgb(241, 245, 249))
            textSize = 16f
            minLines = 2
            maxLines = 6
            gravity = Gravity.TOP or Gravity.START
            setPadding(dp(4), dp(2), dp(4), dp(4))
            setSingleLine(false)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            background = null
            includeFontPadding = false
            setOnFocusChangeListener { v, hasFocus ->
                if (hasFocus) {
                    (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).showSoftInput(v, InputMethodManager.SHOW_IMPLICIT)
                    bridge.appendConnectionEvent("native_input_focus")
                }
            }
            setOnClickListener {
                requestFocus()
                (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
                bridge.appendConnectionEvent("native_input_click")
            }
        }
        composer.addView(nativeInput, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, dp(74)))

        val actionRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(8), 0, 0)
        }
        nativeActionRow = actionRow
        val plus = TextView(this).apply {
            text = "+"
            textSize = 23f
            setTextColor(Color.rgb(212, 217, 226))
            gravity = Gravity.CENTER
            background = rounded(Color.rgb(44, 49, 60), 18)
        }
        actionRow.addView(plus, LinearLayout.LayoutParams(dp(36), dp(36)))

        fun chip(text: String): TextView = TextView(this).apply {
            this.text = text
            textSize = 13f
            setTextColor(Color.rgb(213, 218, 228))
            gravity = Gravity.CENTER
            setPadding(dp(12), 0, dp(12), 0)
            background = rounded(Color.rgb(39, 44, 55), 18)
        }
        actionRow.addView(chip("Build⌄"), LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, dp(36)).apply { leftMargin = dp(8) })
        actionRow.addView(chip("模型⌄"), LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, dp(36)).apply { leftMargin = dp(6) })
        actionRow.addView(chip("默认⌄"), LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, dp(36)).apply { leftMargin = dp(6) })

        val spacer = android.view.View(this)
        actionRow.addView(spacer, LinearLayout.LayoutParams(0, 1, 1f))

        val send = TextView(this).apply {
            text = "↑"
            textSize = 22f
            setTextColor(Color.rgb(15, 18, 24))
            gravity = Gravity.CENTER
            background = rounded(Color.rgb(235, 238, 245), 18)
            setOnClickListener {
                val payload = nativeInput.text.toString().trim()
                if (payload.isNotEmpty()) {
                    val quoted = JSONObject.quote(payload)
                    webView.evaluateJavascript("if(typeof dispatchUserPayload==='function' && dispatchUserPayload($quoted)){true}else{false}") { result ->
                        if (result == "true") nativeInput.setText("")
                    }
                    bridge.appendConnectionEvent("native_input_send len=${payload.length}")
                }
            }
        }
        nativeSendButton = send
        actionRow.addView(send, LinearLayout.LayoutParams(dp(36), dp(36)))
        composer.addView(actionRow, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
        applyNativeInputTheme("finger")
        return outer
    }

    private fun applyNativeInputTheme(theme: String) {
        val light = theme == "sunrise" || theme == "paper"
        val composerBg = if (light) Color.rgb(255, 250, 243) else Color.rgb(17, 24, 39)
        val composerStroke = if (light) Color.rgb(222, 212, 200) else Color.rgb(39, 50, 68)
        val textColor = if (light) Color.rgb(32, 26, 22) else Color.rgb(238, 242, 247)
        val mutedColor = if (light) Color.rgb(117, 106, 95) else Color.rgb(152, 162, 179)
        val chipBg = if (light) Color.rgb(238, 226, 210) else Color.rgb(24, 34, 53)
        val sendBg = if (light) Color.rgb(180, 83, 9) else Color.rgb(122, 162, 247)
        val sendText = if (light) Color.WHITE else Color.rgb(8, 17, 31)
        fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
        fun rounded(color: Int, radiusDp: Int, strokeDp: Int = 0, strokeColor: Int = Color.TRANSPARENT) =
            android.graphics.drawable.GradientDrawable().apply {
                shape = android.graphics.drawable.GradientDrawable.RECTANGLE
                cornerRadius = dp(radiusDp).toFloat()
                setColor(color)
                if (strokeDp > 0) setStroke(dp(strokeDp), strokeColor)
            }
        nativeComposer?.background = rounded(composerBg, 24, 1, composerStroke)
        nativeInput.setTextColor(textColor)
        nativeInput.setHintTextColor(mutedColor)
        nativeActionRow?.let { row ->
            for (i in 0 until row.childCount) {
                val child = row.getChildAt(i)
                if (child is TextView && child !== nativeSendButton) {
                    child.setTextColor(textColor)
                    child.background = rounded(chipBg, 18)
                }
            }
        }
        nativeSendButton?.setTextColor(sendText)
        nativeSendButton?.background = rounded(sendBg, 18)
    }

    private fun applyNativeChromeMode(mode: String) {
        val chromePanelOpen = mode == "sessions" || mode == "settings"
        nativeInputBar?.visibility = if (chromePanelOpen) View.GONE else View.VISIBLE
        if (chromePanelOpen) {
            nativeInput.clearFocus()
            val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
            imm.hideSoftInputFromWindow(nativeInput.windowToken, 0)
        }
    }

    private fun runUpdateSelfTestIfRequested(bridge: MobileBridge) {
        val enabled = intent?.getBooleanExtra("updateSelfTest", false) ?: false
        if (!enabled) return
        val manifestUrl = intent?.getStringExtra("manifestUrl") ?: "internal://latest"
        val baseUrl = intent?.getStringExtra("baseUrl") ?: "internal://files/"
        thread(start = true, name = "update-self-test") {
            bridge.appendConnectionEvent("update.selftest.start manifest=$manifestUrl base=$baseUrl")
            val c = bridge.checkUpdate(manifestUrl)
            bridge.appendConnectionEvent("update.selftest.check=$c")
            val d = bridge.downloadUpdateApk(baseUrl)
            bridge.appendConnectionEvent("update.selftest.download=$d")
            val i = bridge.installDownloadedApk()
            bridge.appendConnectionEvent("update.selftest.install=$i")
            bridge.appendConnectionEvent("update.selftest.done")
        }
    }



    private fun copyUpdateManifestIfPresent() {
        runCatching {
            val source = File("/sdcard/Download/latest.json")
            val targetDir = File(filesDir, "app_update_dist")
            if (!targetDir.exists()) targetDir.mkdirs()
            val target = File(targetDir, "latest.json")
            if (source.exists()) source.copyTo(target, overwrite = true)
        }
    }

    override fun onPause() {
        super.onPause()
        // Save WebView state before going to background
        webView.evaluateJavascript(
            "if(typeof CONFIG !== 'undefined') CONFIG.save();",
            null
        )
    }

    override fun onResume() {
        super.onResume()
        // Restore WebView state after coming back
        webView.evaluateJavascript(
            "if(typeof CONFIG !== 'undefined') CONFIG.load();",
            null
        )
    }

    override fun onDestroy() {
        webView.removeJavascriptInterface("FinMobileBridge")
        webView.destroy()
        super.onDestroy()
    }
}
