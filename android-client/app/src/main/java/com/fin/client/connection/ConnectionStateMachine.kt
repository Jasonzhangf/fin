package com.fin.client.connection

import com.fin.client.model.ConnectionState

class ConnectionStateMachine {
    fun next(current: ConnectionState, action: String): ConnectionState {
        return when (current) {
            ConnectionState.IDLE -> if (action == "scan") ConnectionState.SCANNED else current
            ConnectionState.SCANNED -> if (action == "resolve") ConnectionState.RESOLVING else current
            ConnectionState.RESOLVING -> when (action) {
                "ok" -> ConnectionState.CONNECTING
                "bad_token" -> ConnectionState.STALE_TOKEN
                else -> current
            }
            ConnectionState.CONNECTING -> when (action) {
                "connected" -> ConnectionState.HANDSHAKING
                "unreachable" -> ConnectionState.ENDPOINT_UNREACHABLE
                else -> current
            }
            ConnectionState.HANDSHAKING -> when (action) {
                "auth_ok" -> ConnectionState.SUBSCRIBED
                "auth_failed" -> ConnectionState.AUTH_FAILED
                "protocol_mismatch" -> ConnectionState.PROTOCOL_MISMATCH
                else -> current
            }
            ConnectionState.SUBSCRIBED -> if (action == "healthy") ConnectionState.HEALTHY else current
            else -> current
        }
    }
}
