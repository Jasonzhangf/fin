package com.fin.client.ws

import kotlinx.serialization.Serializable

@Serializable
data class QrConnectPayload(
    val v: Int,
    val endpoint: String,
    val device_id: String,
    val token: String,
    val exp: Long,
    val project: String,
    val scopes: List<String>
)

@Serializable
data class WsEnvelope(
    val event_type: String,
    val trace_id: String? = null,
    val payload: String
)
