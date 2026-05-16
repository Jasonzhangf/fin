package com.fin.client

import com.fin.client.connection.ConnectionStateMachine
import com.fin.client.model.ConnectionState
import org.junit.Assert.assertEquals
import org.junit.Test

class ConnectionStateMachineTest {
    @Test
    fun happyPath() {
        val m = ConnectionStateMachine()
        var s = ConnectionState.IDLE
        s = m.next(s, "scan")
        s = m.next(s, "resolve")
        s = m.next(s, "ok")
        s = m.next(s, "connected")
        s = m.next(s, "auth_ok")
        s = m.next(s, "healthy")
        assertEquals(ConnectionState.HEALTHY, s)
    }
}
