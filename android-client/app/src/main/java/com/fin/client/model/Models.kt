package com.fin.client.model

import kotlinx.serialization.Serializable

enum class ConnectionState {
    IDLE, SCANNED, RESOLVING, CONNECTING, HANDSHAKING, SUBSCRIBED, HEALTHY,
    AUTH_FAILED, ENDPOINT_UNREACHABLE, STALE_TOKEN, PROTOCOL_MISMATCH
}

@Serializable
data class WsProfile(
    val id: String,
    val name: String,
    val endpoint: String,
    val token: String,
    val project: String,
    val expiresAtEpochSec: Long,
)

data class SessionSummary(
    val sessionId: String,
    val taskId: String?,
    val topic: String,
    val phase: String,
    val updatedAt: String,
    val project: String,
)

enum class InputState { DRAFT, QUEUED, CONSUMED, RENDERED, DELIVERED }

data class InputItem(
    val source: String,
    val clientMessageId: String,
    val timestampBucket: Long,
    val payload: String,
    val state: InputState,
) {
    val dedupeKey: String get() = "$source:$clientMessageId:$timestampBucket"
}

data class TurnCard(
    val turnId: String,
    val userInput: String,
    val assistantResponse: String,
    val controlFeedbackSummary: String?,
    val toolExecutionSummary: String?,
    val closureStopSource: String?,
)

data class WorkerStatus(
    val workerId: String,
    val presence: String,
    val heartbeatAt: String?,
    val currentTaskId: String?,
)

data class ProjectStatus(
    val projectId: String,
    val supervisionAction: String?,
    val wakeQueueState: String?,
    val pickupState: String?,
)

@Serializable
data class ProviderProfile(
    val profile_name: String,
    val provider: String,
    val protocol: String,
    val base_url: String = "",
    val model: String,
    val credential_source: String = "",
    val active: Boolean,
)

@Serializable
data class ProviderConfigCache(
    val cache_kind: String = "runtime_config_snapshot",
    val default_profile: String,
    val profiles: List<ProviderProfile>,
    val active_thinking_effort: String? = null,
)
