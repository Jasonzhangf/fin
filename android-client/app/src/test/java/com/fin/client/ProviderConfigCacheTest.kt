package com.fin.client

import com.fin.client.model.ProviderConfigCache
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ProviderConfigCacheTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun serverRuntimeSnapshotParsesWithoutLegacyCredentialFields() {
        val snapshot = """
            {
              "type": "config.snapshot",
              "status": "ok",
              "default_profile": "minimax",
              "active_thinking_effort": null,
              "profiles": [{
                "profile_name": "minimax",
                "provider": "minimax",
                "protocol": "anthropic-wire",
                "model": "MiniMax-M3",
                "active": true
              }]
            }
        """.trimIndent()

        val parsed = json.decodeFromString<ProviderConfigCache>(snapshot)
        val encoded = json.encodeToString(parsed)

        assertEquals("minimax", parsed.default_profile)
        assertNull(parsed.active_thinking_effort)
        assertEquals("minimax", parsed.profiles.single().provider)
        assertEquals("MiniMax-M3", parsed.profiles.single().model)
        assertEquals("", parsed.profiles.single().base_url)
        assertEquals("", parsed.profiles.single().credential_source)
        assert(encoded.contains("MiniMax-M3"))
    }
}
