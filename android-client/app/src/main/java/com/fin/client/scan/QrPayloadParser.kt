package com.fin.client.scan

import com.fin.client.ws.QrConnectPayload
import kotlinx.serialization.json.Json

object QrPayloadParser {
    fun parse(raw: String): QrConnectPayload = Json.decodeFromString(raw)
}
