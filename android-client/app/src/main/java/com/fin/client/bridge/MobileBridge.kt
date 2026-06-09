package com.fin.client.bridge

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.content.FileProvider
import com.fin.client.scan.QrPayloadParser
import com.fin.client.storage.WsProfileStore
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import android.util.Log
import android.view.inputmethod.InputMethodManager
import java.io.File
import java.io.FileOutputStream
import java.net.InetSocketAddress
import java.net.Proxy
import java.net.Socket
import java.net.URI
import java.security.MessageDigest
import java.time.Instant
import android.util.Base64
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONObject

class MobileBridge(
    private val context: Context,
    private val profileStore: WsProfileStore,
    private val nativeThemeApplier: ((String) -> Unit)? = null,
    private val nativeChromeModeApplier: ((String) -> Unit)? = null,
) {
    private val tag = "FinMobileBridge"
    private var webView: WebView? = null
    private var nativeWebSocket: WebSocket? = null
    private var nativeWsGeneration: Long = 0
    private val httpClient = OkHttpClient.Builder()
        .proxy(Proxy.NO_PROXY)
        .connectTimeout(20, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

    fun attachWebView(view: WebView) {
        webView = view
    }

    @JavascriptInterface
    fun showKeyboard(): String {
        return runCatching {
            val imm = context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
            imm.toggleSoftInput(InputMethodManager.SHOW_FORCED, 0)
            appendConnectionEvent("ime_show_requested")
            "ok"
        }.getOrElse { "error:${it.message ?: "unknown"}" }
    }

    @JavascriptInterface
    fun getWsProfiles(): String = profileStore.readAllJson()

    @JavascriptInterface
    fun saveWsProfile(payloadJson: String): String {
        profileStore.saveFromJson(payloadJson)
        return "ok"
    }

    @JavascriptInterface
    fun setDaemonAddress(host: String, port: Int): String {
        profileStore.setDaemonAddress(host, port)
        return "ok"
    }

    @JavascriptInterface
    fun applyNativeTheme(theme: String): String {
        nativeThemeApplier?.invoke(theme)
        return "ok"
    }

    @JavascriptInterface
    fun applyNativeChromeMode(mode: String): String {
        nativeChromeModeApplier?.invoke(mode)
        return "ok"
    }

    @JavascriptInterface
    fun nativeWsConnect(endpoint: String, token: String, project: String): String {
        return runCatching {
            nativeWsGeneration += 1
            val generation = nativeWsGeneration
            nativeWebSocket?.close(1000, "replace_connection")
            val request = Request.Builder().url(endpoint).build()
            nativeWebSocket = httpClient.newWebSocket(request, object : WebSocketListener() {
                private fun isCurrent(): Boolean = generation == nativeWsGeneration

                override fun onOpen(webSocket: WebSocket, response: okhttp3.Response) {
                    if (!isCurrent()) {
                        appendConnectionEvent("native_ws.stale_open generation=$generation")
                        webSocket.close(1000, "stale_connection")
                        return
                    }
                    appendConnectionEvent("native_ws.open endpoint=$endpoint")
                    val handshake = JSONObject().apply {
                        put("type", "mobile.handshake")
                        put("token", token)
                        put("project", project)
                        put("scopes", org.json.JSONArray(listOf("session.read", "session.write", "runtime.read")))
                    }
                    webSocket.send(handshake.toString())
                    emitNativeWsState("handshaking")
                }

                override fun onMessage(webSocket: WebSocket, text: String) {
                    if (!isCurrent()) {
                        appendConnectionEvent("native_ws.stale_message generation=$generation")
                        if (isTurnResultFrame(text)) {
                            appendConnectionEvent("native_ws.forward_stale_turn generation=$generation")
                            emitNativeWsMessage(text)
                        }
                        return
                    }
                    emitNativeWsMessage(text)
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: okhttp3.Response?) {
                    val detail = t.message ?: "unknown_failure"
                    if (!isCurrent()) {
                        appendConnectionEvent("native_ws.stale_failure generation=$generation detail=$detail")
                        return
                    }
                    appendConnectionEvent("native_ws.failure endpoint=$endpoint detail=$detail")
                    emitNativeWsState("endpoint_unreachable:$detail")
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    if (!isCurrent()) {
                        appendConnectionEvent("native_ws.stale_closed code=$code reason=$reason generation=$generation")
                        return
                    }
                    appendConnectionEvent("native_ws.closed code=$code reason=$reason")
                    emitNativeWsState("closed")
                }
            })
            "ok"
        }.getOrElse {
            val detail = it.message ?: "unknown"
            appendConnectionEvent("native_ws.connect_error endpoint=$endpoint detail=$detail")
            "error:$detail"
        }
    }

    @JavascriptInterface
    fun nativeWsSend(payloadJson: String): String {
        return if (nativeWebSocket?.send(payloadJson) == true) "ok" else "error:native_ws_not_connected"
    }

    @JavascriptInterface
    fun nativeWsClose(): String {
        nativeWsGeneration += 1
        nativeWebSocket?.close(1000, "client_close")
        nativeWebSocket = null
        return "ok"
    }


    @JavascriptInterface
    fun saveProviderConfigCache(payloadJson: String): String {
        profileStore.saveProviderConfigCache(payloadJson)
        return "ok"
    }

    @JavascriptInterface
    fun readProviderConfigCacheJson(): String {
        return profileStore.readProviderConfigCacheJson() ?: ""
    }

    @JavascriptInterface
    fun parseQrPayload(raw: String): String {
        val payload = QrPayloadParser.parse(raw)
        return """
            {"ok":true,"endpoint":"${payload.endpoint}","project":"${payload.project}","exp":${payload.exp}}
        """.trimIndent()
    }

    @JavascriptInterface
    fun readLatestJson(): String {
        val f = File(context.filesDir, "app_update_dist/latest.json")
        return if (f.exists()) f.readText() else """{"error":"latest.json_not_found"}"""
    }

    @JavascriptInterface
    fun checkUpdate(manifestUrl: String): String {
        Log.i(tag, "checkUpdate called manifestUrl=$manifestUrl")
        return runCatching {
            if (manifestUrl == "internal://latest") {
                val src = File(context.filesDir, "app_update_dist/latest.json")
                if (!src.exists()) return """{"ok":false,"error":"manifest_file_not_found"}"""
                val body = src.readText()
                if (body.isBlank()) return """{"ok":false,"error":"empty_manifest"}"""
                return """{"ok":true,"saved":"${src.absolutePath}"}"""
            }
            if (manifestUrl.startsWith("file://")) {
                val src = File(URI(manifestUrl))
                if (!src.exists()) return """{"ok":false,"error":"manifest_file_not_found"}"""
                val body = src.readText()
                if (body.isBlank()) return """{"ok":false,"error":"empty_manifest"}"""
                val dir = File(context.filesDir, "app_update_dist")
                if (!dir.exists()) dir.mkdirs()
                val target = File(dir, "latest.json")
                target.writeText(body)
                return """{"ok":true,"saved":"${target.absolutePath}"}"""
            }
            val req = Request.Builder().url(manifestUrl).build()
            httpClient.newCall(req).execute().use { resp ->
                if (!resp.isSuccessful) {
                    return """{"ok":false,"error":"http_${resp.code}"}"""
                }
                val body = resp.body?.string().orEmpty()
                if (body.isBlank()) return """{"ok":false,"error":"empty_manifest"}"""
                val dir = File(context.filesDir, "app_update_dist")
                if (!dir.exists()) dir.mkdirs()
                val target = File(dir, "latest.json")
                target.writeText(body)
                Log.i(tag, "checkUpdate ok saved=${target.absolutePath}")
                """{"ok":true,"saved":"${target.absolutePath}"}"""
            }
        }.getOrElse {
            Log.e(tag, "checkUpdate failed", it)
            """{"ok":false,"error":"${it.message?.replace("\"","'") ?: "unknown"}"}"""
        }
    }

    @JavascriptInterface
    fun downloadUpdateApk(baseUrl: String): String {
        Log.i(tag, "downloadUpdateApk called baseUrl=$baseUrl")
        return runCatching {
            val latest = File(context.filesDir, "app_update_dist/latest.json")
            if (!latest.exists()) return """{"ok":false,"error":"latest_json_missing"}"""
            val obj = JSONObject(latest.readText())
            val apkUrlRaw = obj.optString("apkUrl", "")
            if (apkUrlRaw.isBlank()) return """{"ok":false,"error":"apk_url_missing"}"""
            val expectedSize = obj.optLong("size", -1L)
            val expectedSha256 = obj.optString("sha256", "").trim().lowercase()
            val resolved = if (apkUrlRaw.startsWith("http://") || apkUrlRaw.startsWith("https://") || apkUrlRaw.startsWith("file://")) {
                apkUrlRaw
            } else {
                if (baseUrl == "internal://files/") {
                    "internal://files/$apkUrlRaw"
                } else {
                    val base = if (baseUrl.endsWith("/")) baseUrl else "$baseUrl/"
                    "$base$apkUrlRaw"
                }
            }
            Log.i(tag, "downloadUpdateApk resolved=$resolved")
            if (resolved.startsWith("internal://files/")) {
                val dir = File(context.filesDir, "app_update_dist")
                if (!dir.exists()) dir.mkdirs()
                val apk = File(dir, "fin-latest-debug.apk")
                val fileName = resolved.removePrefix("internal://files/")
                val srcInInternal = File(dir, fileName)
                if (srcInInternal.exists()) {
                    srcInInternal.copyTo(apk, overwrite = true)
                    return validateDownloadedApk(apk, expectedSize, expectedSha256, "internal_file")
                }
                val srcTmp = File("/data/local/tmp/$fileName")
                if (srcTmp.exists()) {
                    srcTmp.copyTo(apk, overwrite = true)
                    return validateDownloadedApk(apk, expectedSize, expectedSha256, "tmp_file")
                }
                return """{"ok":false,"error":"apk_file_not_found"}"""
            }
            if (resolved.startsWith("file://")) {
                val src = File(URI(resolved))
                if (!src.exists()) return """{"ok":false,"error":"apk_file_not_found"}"""
                val dir = File(context.filesDir, "app_update_dist")
                if (!dir.exists()) dir.mkdirs()
                val apk = File(dir, "fin-latest-debug.apk")
                src.copyTo(apk, overwrite = true)
                return validateDownloadedApk(apk, expectedSize, expectedSha256, "file")
            }
            val req = Request.Builder().url(resolved).build()
            httpClient.newCall(req).execute().use { resp ->
                if (!resp.isSuccessful) {
                    return """{"ok":false,"error":"http_${resp.code}"}"""
                }
                val bytes = resp.body?.bytes() ?: ByteArray(0)
                if (bytes.isEmpty()) return """{"ok":false,"error":"empty_apk"}"""
                val dir = File(context.filesDir, "app_update_dist")
                if (!dir.exists()) dir.mkdirs()
                val apk = File(dir, "fin-latest-debug.apk")
                FileOutputStream(apk).use { it.write(bytes) }
                Log.i(tag, "downloadUpdateApk ok apk=${apk.absolutePath} size=${apk.length()}")
                validateDownloadedApk(apk, expectedSize, expectedSha256, "http")
            }
        }.getOrElse {
            Log.e(tag, "downloadUpdateApk failed", it)
            """{"ok":false,"error":"${it.message?.replace("\"","'") ?: "unknown"}"}"""
        }
    }

    @JavascriptInterface
    fun installDownloadedApk(): String {
        Log.i(tag, "installDownloadedApk called")
        return runCatching {
            val apk = File(context.filesDir, "app_update_dist/fin-latest-debug.apk")
            if (!apk.exists()) return """{"ok":false,"error":"apk_missing"}"""
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !context.packageManager.canRequestPackageInstalls()) {
                val settingsIntent = Intent(
                    Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:${context.packageName}")
                ).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                context.startActivity(settingsIntent)
                return """{"ok":false,"error":"install_permission_required","settings_launched":true}"""
            }
            val uri: Uri = FileProvider.getUriForFile(
                context,
                "${context.packageName}.fileprovider",
                apk
            )
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            context.startActivity(intent)
            Log.i(tag, "installDownloadedApk launched uri=$uri")
            """{"ok":true,"launched":true}"""
        }.getOrElse {
            Log.e(tag, "installDownloadedApk failed", it)
            """{"ok":false,"error":"${it.message?.replace("\"","'") ?: "unknown"}"}"""
        }
    }

    private fun validateDownloadedApk(apk: File, expectedSize: Long, expectedSha256: String, source: String): String {
        if (!apk.exists()) return """{"ok":false,"error":"apk_file_not_found"}"""
        val actualSize = apk.length()
        if (expectedSize >= 0L && actualSize != expectedSize) {
            return """{"ok":false,"error":"apk_size_mismatch","expected":$expectedSize,"actual":$actualSize}"""
        }
        if (expectedSha256.isNotBlank()) {
            val actualSha256 = sha256Hex(apk)
            if (actualSha256 != expectedSha256) {
                return """{"ok":false,"error":"apk_sha256_mismatch","expected":"$expectedSha256","actual":"$actualSha256"}"""
            }
        }
        return """{"ok":true,"apk":"${apk.absolutePath}","size":$actualSize,"source":"$source"}"""
    }

    private fun sha256Hex(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            while (true) {
                val read = input.read(buffer)
                if (read <= 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun isTurnResultFrame(text: String): Boolean {
        return runCatching {
            val type = JSONObject(text).optString("type", "")
            type == "input.accepted" ||
                type == "turn.started" ||
                type == "turn.progress" ||
                type == "turn.item.started" ||
                type == "turn.item.delta" ||
                type == "turn.item.completed" ||
                type == "turn.item.failed" ||
                type == "turn.completed" ||
                type == "turn.trace_event" ||
                type == "turn.rendered"
        }.getOrDefault(false)
    }

    @JavascriptInterface
    fun getConfigPath(): String = File(context.filesDir, "config/ws_profiles.json").absolutePath

    @JavascriptInterface
    fun appendConnectionEvent(event: String): String {
        val dir = File(context.filesDir, "logs")
        if (!dir.exists()) dir.mkdirs()
        val f = File(dir, "connection-events.log")
        f.appendText("${Instant.now()} ${event}\n")
        Log.i(tag, event)
        forwardConnectionEventToDaemon(event)
        return "ok"
    }

    private fun forwardConnectionEventToDaemon(event: String) {
        Thread {
            runCatching {
                val daemon = profileStore.readAll().firstOrNull { it.id == "daemon" } ?: return@runCatching
                val endpoint = daemon.endpoint
                val hostPort = endpoint.removePrefix("ws://").removePrefix("wss://").substringBefore("/")
                val url = "http://$hostPort/api/log/ingest"
                val bodyJson = JSONObject().apply {
                    put("ts", Instant.now().toString())
                    put("source", "android")
                    put("event", event)
                }.toString()
                val req = Request.Builder()
                    .url(url)
                    .post(bodyJson.toRequestBody("application/json; charset=utf-8".toMediaType()))
                    .build()
                httpClient.newCall(req).execute().use { resp ->
                    if (!resp.isSuccessful) {
                        Log.w(tag, "forwardConnectionEventToDaemon failed code=${resp.code}")
                    }
                }
            }.onFailure {
                Log.w(tag, "forwardConnectionEventToDaemon error", it)
            }
        }.start()
    }

    @JavascriptInterface
    fun readConnectionEvents(): String {
        val f = File(context.filesDir, "logs/connection-events.log")
        return if (f.exists()) f.readText() else ""
    }

    @JavascriptInterface
    fun providerCacheDebug(): String {
        return runCatching {
            val filesDirPath = context.filesDir.absolutePath
            val cacheFile = File(context.filesDir, "config/runtime_config_snapshot.json")
            val exists = cacheFile.exists()
            val size = if (exists) cacheFile.length() else 0L
            val preview = if (exists) {
                cacheFile.readText().take(400).replace("\n", "\\n")
            } else {
                ""
            }
            """{"ok":true,"files_dir":"${filesDirPath.replace("\"", "'\"")}","cache_path":"${cacheFile.absolutePath.replace("\"", "'\"")}","exists":$exists,"size":$size,"preview":"${preview.replace("\"", "'\"")}"}"""
        }.getOrElse {
            """{"ok":false,"error":"${it.message?.replace("\"", "'\"") ?: "unknown"}"}"""
        }
    }

    @JavascriptInterface
    fun probeWs(endpoint: String): String {
        return runCatching {
            val uri = URI(endpoint)
            val host = uri.host ?: return """{"ok":false,"error":"invalid_host"}"""
            val port = if (uri.port > 0) uri.port else 80
            val path = if (uri.rawPath.isNullOrBlank()) "/" else uri.rawPath
            val socket = Socket(Proxy.NO_PROXY)
            socket.connect(InetSocketAddress(host, port), 10000)
            socket.soTimeout = 10000
            val out = socket.getOutputStream()
            val `in` = socket.getInputStream()
            val req = buildString {
                append("GET $path HTTP/1.1\r\n")
                append("Host: $host:$port\r\n")
                append("Upgrade: websocket\r\n")
                append("Connection: Upgrade\r\n")
                append("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n")
                append("Sec-WebSocket-Version: 13\r\n")
                append("\r\n")
            }
            out.write(req.toByteArray())
            out.flush()
            val buf = ByteArray(1024)
            val n = `in`.read(buf)
            socket.close()
            val head = if (n > 0) String(buf, 0, n) else ""
            val status = head.lineSequence().firstOrNull().orEmpty()
            """{"ok":true,"host":"$host","port":$port,"path":"$path","status":"${status.replace("\"","'")}"}"""
        }.getOrElse {
            Log.e(tag, "probeWs failed endpoint=$endpoint", it)
            """{"ok":false,"error":"${it.message?.replace("\"","'") ?: "unknown"}"}"""
        }
    }

    @JavascriptInterface
    fun probeOkHttpWs(endpoint: String): String {
        return runCatching {
            val latch = CountDownLatch(1)
            var status = "unknown"
            var detail = ""
            val client = OkHttpClient.Builder()
                .proxy(Proxy.NO_PROXY)
                .connectTimeout(10, TimeUnit.SECONDS)
                .readTimeout(10, TimeUnit.SECONDS)
                .build()
            val request = Request.Builder().url(endpoint).build()
            val ws: WebSocket = client.newWebSocket(request, object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: okhttp3.Response) {
                    status = "open"
                    detail = response.message
                    webSocket.close(1000, "probe_done")
                    latch.countDown()
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: okhttp3.Response?) {
                    status = "failed"
                    detail = t.message ?: "unknown_failure"
                    latch.countDown()
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    if (status == "unknown") {
                        status = "closed"
                        detail = "$code:$reason"
                    }
                    latch.countDown()
                }
            })
            val done = latch.await(12, TimeUnit.SECONDS)
            if (!done && status == "unknown") {
                status = "timeout"
                detail = "await_timeout"
                ws.cancel()
            }
            client.dispatcher.executorService.shutdown()
            """{"ok":${status == "open"},"status":"$status","detail":"${detail.replace("\"","'")}"}"""
        }.getOrElse {
            Log.e(tag, "probeOkHttpWs failed endpoint=$endpoint", it)
            """{"ok":false,"status":"error","detail":"${it.message?.replace("\"","'") ?: "unknown"}"}"""
        }
    }

    @JavascriptInterface
    fun probeHttp(endpoint: String): String {
        return runCatching {
            val uri = URI(endpoint)
            val host = uri.host ?: return """{"ok":false,"error":"invalid_host"}"""
            val port = if (uri.port > 0) uri.port else 80
            val path = if (uri.rawPath.isNullOrBlank()) "/" else uri.rawPath
            val socket = Socket(Proxy.NO_PROXY)
            socket.connect(InetSocketAddress(host, port), 10000)
            socket.soTimeout = 10000
            val out = socket.getOutputStream()
            val `in` = socket.getInputStream()
            val req = "GET $path HTTP/1.1\r\nHost: $host:$port\r\nConnection: close\r\n\r\n"
            out.write(req.toByteArray())
            out.flush()
            val buf = ByteArray(512)
            val n = `in`.read(buf)
            socket.close()
            val status = if (n > 0) String(buf, 0, n).lineSequence().firstOrNull().orEmpty() else ""
            """{"ok":true,"status":"${status.replace("\"","'")}"}"""
        }.getOrElse {
            Log.e(tag, "probeHttp failed endpoint=$endpoint", it)
            """{"ok":false,"error":"${it.message?.replace("\"","'") ?: "unknown"}"}"""
        }
    }

    @JavascriptInterface
    fun getDeviceInfo(): String = """{"platform":"android","mode":"webui-reuse"}"""

    private fun emitNativeWsMessage(text: String) {
        val encoded = Base64.encodeToString(text.toByteArray(Charsets.UTF_8), Base64.NO_WRAP)
        val quoted = JSONObject.quote(encoded)
        emitJavascript("window.onNativeWsMessageB64 && window.onNativeWsMessageB64($quoted);")
    }

    private fun emitNativeWsState(state: String) {
        val quoted = JSONObject.quote(state)
        emitJavascript("window.onNativeWsState && window.onNativeWsState($quoted);")
    }

    private fun emitJavascript(script: String) {
        webView?.post { webView?.evaluateJavascript(script, null) }
    }
}
