package com.fin.client

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class MobileShellLayoutContractTest {
    private val shell = File("src/main/assets/mobile-shell.html").readText()
    private val activity = File("src/main/java/com/fin/client/MainActivity.kt").readText()

    @Test
    fun webConversationStartsAtTopAndKeepsBottomInsetForNativeComposer() {
        assertTrue(shell.contains(".content{flex:1;padding:78px 0 var(--native-input-inset,96px);display:flex;align-items:flex-start;justify-content:flex-start"))
        assertTrue(shell.contains("body.native-input .content{padding-bottom:var(--native-input-inset,150px)}"))
        assertTrue(shell.contains("function setNativeInputInset(px)"))
    }

    @Test
    fun nativeComposerTracksImeAndReportsCombinedInset() {
        assertTrue(activity.contains("ViewCompat.setOnApplyWindowInsetsListener"))
        assertTrue(activity.contains("WindowInsetsCompat.Type.ime()"))
        assertTrue(activity.contains("view.translationY = -imeBottom.toFloat()"))
        assertTrue(activity.contains("val cssPx = ((barHeight + keyboardHeight) / density).toInt().coerceAtLeast(120)"))
    }

    @Test
    fun decorativeBlueDividerLinesAreNotRenderedInConversationOrChips() {
        assertTrue(shell.contains(".timeline{margin-top:8px;padding-top:4px}"))
        assertFalse(shell.contains(".timeline{margin-top:8px;border-top:1px dashed var(--line);padding-top:8px}"))
        assertTrue(shell.contains(".tl-item{font-size:11px;color:var(--text);line-height:1.45;margin:5px 0;padding:6px 8px;border-radius:10px;background:var(--chip);border:none}"))
        assertTrue(activity.contains("background = rounded(Color.rgb(39, 44, 55), 18)"))
        assertFalse(activity.contains("background = rounded(Color.rgb(39, 44, 55), 18, 1, Color.rgb(63, 70, 84))"))
    }
}
