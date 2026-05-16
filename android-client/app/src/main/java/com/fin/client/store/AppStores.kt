package com.fin.client.store

import com.fin.client.model.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

class ConnectionStore {
    private val _state = MutableStateFlow(ConnectionState.IDLE)
    val state: StateFlow<ConnectionState> = _state
    fun set(value: ConnectionState) { _state.value = value }
}

class SessionStore {
    private val _sessions = MutableStateFlow<List<SessionSummary>>(emptyList())
    private val _currentSessionId = MutableStateFlow<String?>(null)
    val sessions: StateFlow<List<SessionSummary>> = _sessions
    val currentSessionId: StateFlow<String?> = _currentSessionId

    fun replace(all: List<SessionSummary>) { _sessions.value = all }
    fun bind(sessionId: String) { _currentSessionId.value = sessionId }
}

class InputStore {
    private val _inputs = MutableStateFlow<List<InputItem>>(emptyList())
    val inputs: StateFlow<List<InputItem>> = _inputs

    fun enqueue(item: InputItem) {
        if (_inputs.value.any { it.dedupeKey == item.dedupeKey }) return
        _inputs.value = _inputs.value + item
    }

    fun transition(clientMessageId: String, newState: InputState) {
        _inputs.value = _inputs.value.map {
            if (it.clientMessageId == clientMessageId) it.copy(state = newState) else it
        }
    }
}

class TurnStore {
    private val _turns = MutableStateFlow<List<TurnCard>>(emptyList())
    val turns: StateFlow<List<TurnCard>> = _turns
    fun append(turn: TurnCard) { _turns.value = _turns.value + turn }
}

class RuntimeStore {
    private val _workers = MutableStateFlow<List<WorkerStatus>>(emptyList())
    private val _projects = MutableStateFlow<List<ProjectStatus>>(emptyList())
    val workers: StateFlow<List<WorkerStatus>> = _workers
    val projects: StateFlow<List<ProjectStatus>> = _projects
    fun setWorkers(v: List<WorkerStatus>) { _workers.value = v }
    fun setProjects(v: List<ProjectStatus>) { _projects.value = v }
}
