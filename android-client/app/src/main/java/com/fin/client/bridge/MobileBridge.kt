package com.fin.client.bridge

import android.content.Context
import android.webkit.JavascriptInterface
import com.fin.client.scan.QrPayloadParser
import com.fin.client.storage.WsProfileStore
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import android.util.Log
import java.io.File
import java.net.InetSocketAddress
import java.net.Proxy
import java.net.Socket
import java.net.URI
import java.time.Instant
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class MobileBridge(
    private val context: Context,
    private val profileStore: WsProfileStore,
) {
    private val tag = "FinMobileBridge"
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
    fun parseQrPayload(raw: String): String {
        val payload = QrPayloadParser.parse(raw)
        return """
            {"ok":true,"endpoint":"${payload.endpoint}","project":"${payload.project}","exp":${payload.exp}}
        """.trimIndent()
    }

    @JavascriptInterface
    fun readLatestJson(): String {
        val f = File(context.filesDir.parentFile, "app_update_dist/latest.json")
        return if (f.exists()) f.readText() else """{"error":"latest.json_not_found"}"""
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
        return "ok"
    }

    @JavascriptInterface
    fun readConnectionEvents(): String {
        val f = File(context.filesDir, "logs/connection-events.log")
        return if (f.exists()) f.readText() else ""
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
