package com.fin.client.storage

import android.content.Context
import com.fin.client.model.WsProfile
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

class WsProfileStore(context: Context) {
    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }
    private val cfgDir = File(context.filesDir, "config")
    private val cfgFile = File(cfgDir, "ws_profiles.json")

    fun readAll(): List<WsProfile> {
        ensureSeed()
        val raw = cfgFile.readText()
        return runCatching { json.decodeFromString<List<WsProfile>>(raw) }.getOrElse { defaults() }
    }

    fun readAllJson(): String = json.encodeToString(readAll())

    fun saveFromJson(payloadJson: String) {
        val item = json.decodeFromString<WsProfile>(payloadJson)
        val next = readAll().filterNot { it.id == item.id } + item
        persist(next)
    }

    fun setDaemonAddress(host: String, port: Int = 4040) {
        val endpoint = "ws://$host:$port/ws"
        val next = readAll().filterNot { it.id == "daemon" } + WsProfile(
            id = "daemon",
            name = "Daemon",
            endpoint = endpoint,
            token = "",
            project = "fin",
            expiresAtEpochSec = 0,
        )
        persist(next)
    }

    private fun ensureSeed() {
        if (!cfgDir.exists()) cfgDir.mkdirs()
        if (!cfgFile.exists()) persist(defaults())
    }

    private fun persist(list: List<WsProfile>) {
        if (!cfgDir.exists()) cfgDir.mkdirs()
        cfgFile.writeText(json.encodeToString(list))
    }

    private fun defaults(): List<WsProfile> = listOf(
        WsProfile("local", "Local", "ws://127.0.0.1:4040/ws", "", "fin", 0),
        WsProfile("dev", "Dev", "ws://192.168.1.10:4040/ws", "", "fin", 0),
    )
}
