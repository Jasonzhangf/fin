package com.fin.client.bridge

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.webkit.JavascriptInterface
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
import java.time.Instant
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONObject

class MobileBridge(
    private val context: Context,
    private val profileStore: WsProfileStore,
) {
    private val tag = "FinMobileBridge"
    private val httpClient = OkHttpClient.Builder()
        .proxy(Proxy.NO_PROXY)
        .connectTimeout(20, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

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
                if (!src.exists()) {
                    val fallback = JSONObject().apply {
                        put("versionName", "internal")
                        put("versionCode", 1)
                        put("apkUrl", "fin-latest-debug.apk")
                        put("channel", "internal")
                    }.toString()
                    src.parentFile?.let { parent ->
                        if (!parent.exists()) parent.mkdirs()
                    }
                    src.writeText(fallback)
                }
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
                    return """{"ok":true,"apk":"${apk.absolutePath}","size":${apk.length()},"source":"internal_file"}"""
                }
                val srcTmp = File("/data/local/tmp/$fileName")
                if (srcTmp.exists()) {
                    srcTmp.copyTo(apk, overwrite = true)
                    return """{"ok":true,"apk":"${apk.absolutePath}","size":${apk.length()},"source":"tmp_file"}"""
                }
                val installedApk = File(context.packageCodePath)
                if (installedApk.exists()) {
                    installedApk.copyTo(apk, overwrite = true)
                    return """{"ok":true,"apk":"${apk.absolutePath}","size":${apk.length()},"source":"package_code_path"}"""
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
                return """{"ok":true,"apk":"${apk.absolutePath}","size":${apk.length()}}"""
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
                """{"ok":true,"apk":"${apk.absolutePath}","size":${apk.length()}}"""
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
            val cacheFile = File(context.filesDir, "config/provider_config_cache.json")
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
}
