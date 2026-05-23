package com.fin.client.storage

import android.content.Context
import com.fin.client.model.WsProfile
import com.fin.client.model.ProviderConfigCache
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

class WsProfileStore(context: Context) {
    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }
    private val cfgDir = File(context.filesDir, "config")
    private val cfgFile = File(cfgDir, "ws_profiles.json")

    private val providerCfgFile = File(cfgDir, "provider_config_cache.json")

    fun readAll(): List<WsProfile> {
        ensureSeed()
        val raw = cfgFile.readText()
        val decoded = runCatching { json.decodeFromString<List<WsProfile>>(raw) }.getOrElse { defaults() }
        return normalize(decoded)
    }

    fun readAllJson(): String = json.encodeToString(readAll())

    fun saveFromJson(payloadJson: String) {
        val item = json.decodeFromString<WsProfile>(payloadJson)
        val next = readAll().filterNot { it.id == item.id } + item
        persist(next)
    }



    fun saveProviderConfigCache(payloadJson: String) {
        if (!cfgDir.exists()) cfgDir.mkdirs()
        providerCfgFile.writeText(payloadJson)
    }

    fun readProviderConfigCacheJson(): String? {
        if (!providerCfgFile.exists()) return null
        val raw = providerCfgFile.readText()
        return runCatching {
            val parsed = json.decodeFromString<ProviderConfigCache>(raw)
            json.encodeToString(parsed)
        }.getOrNull()
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
        cfgFile.writeText(json.encodeToString(normalize(list)))
    }

    private fun defaults(): List<WsProfile> = listOf(
        WsProfile("daemon", "Daemon", "ws://100.66.1.82:4040/ws", "", "fin", 0),
    )

    private fun normalize(list: List<WsProfile>): List<WsProfile> {
        val unique = LinkedHashMap<String, WsProfile>()
        list.forEach { unique[it.id] = it }
        if (!unique.containsKey("daemon")) {
            unique["daemon"] = defaults().first()
        }
        val daemon = unique.remove("daemon")!!
        return listOf(daemon) + unique.values.toList()
    }
}
