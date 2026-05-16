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
