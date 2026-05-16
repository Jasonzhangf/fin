package com.fin.client

import com.fin.client.model.InputItem
import com.fin.client.model.InputState
import com.fin.client.store.InputStore
import org.junit.Assert.assertEquals
import org.junit.Test

class InputDedupeTest {
    @Test
    fun dedupe() {
        val store = InputStore()
        val item = InputItem("qq", "m1", 123L, "hello", InputState.QUEUED)
        store.enqueue(item)
        store.enqueue(item)
        assertEquals(1, store.inputs.value.size)
    }
}
