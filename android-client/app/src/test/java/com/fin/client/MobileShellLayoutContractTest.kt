package com.fin.client

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class MobileShellLayoutContractTest {
    private val shell = File("src/main/assets/mobile-shell.html").readText()
    private val activity = File("src/main/java/com/fin/client/MainActivity.kt").readText()
    private val bridge = File("src/main/java/com/fin/client/bridge/MobileBridge.kt").readText()

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

    @Test
    fun inputFocusMustNotHideHistoryAndPageMustRemainVerticallyScrollable() {
        assertTrue(shell.contains(".thread-history,.thread-live{display:flex;flex-direction:column;gap:10px;width:100%;max-width:100%;flex-shrink:0}"))
        assertTrue(shell.contains(".content{flex:1;padding:78px 0 var(--native-input-inset,96px);display:flex;align-items:flex-start;justify-content:flex-start;flex-direction:column;text-align:left"))
        assertTrue(shell.contains("overflow-y:auto;overflow-x:hidden"))
        assertTrue(shell.contains("function scrollToBottom(force){const content=document.getElementById('content');if(!content)return;const doPin=!!force||(!inputFocused()&&!S.userScrolledUp&&!S.userTouchScrolling);"))
        assertTrue(shell.contains("function setupScrollTracking(){const content=document.getElementById('content');if(!content)return;"))
        assertTrue(shell.contains("const syncBottomState=()=>{const atBottom=content.scrollHeight-content.scrollTop-content.clientHeight<80;S.userScrolledUp=!atBottom}"))
        assertTrue(shell.contains("content.addEventListener('touchstart',function(ev){S.userTouchScrolling=true;"))
        assertTrue(shell.contains("const releaseTouchScroll=()=>{S.userTouchScrolling=false;syncBottomState()}"))
        assertTrue(shell.contains("if(forceHistory){scrollToBottom(true);return;}scrollToBottom(false)"))
        assertFalse(shell.contains("function scrollToBottom(){if(inputFocused())return;"))
        assertFalse(shell.contains("if(!inputFocused())scrollToBottom(false)"))
        assertFalse(shell.contains("window.scrollTo(0,document.body.scrollHeight)"))
    }

    @Test
    fun updateSettingsUseCanonicalDaemonUpdatesManifestPath() {
        assertTrue(shell.contains("function canonicalUpdateManifestUrl(host,port)"))
        assertTrue(shell.contains("return 'http://'+h+':'+p+'/updates/latest.json'"))
        assertTrue(shell.contains("function normalizeUpdateManifestUrl(raw)"))
        assertTrue(shell.contains("u.pathname==='/upgrade/manifest.js'||u.pathname==='/upgrade/manifest.json'"))
        assertTrue(shell.contains("return u.origin+'/updates/latest.json'"))
        assertTrue(shell.contains("CONFIG.set('update_manifest_url',canonicalUpdateManifestUrl(host,port))"))
        assertTrue(shell.contains("syncDaemonInputsFromConfig();"))
        assertFalse(shell.contains("+'/upgrade/manifest.js'"))
        assertFalse(shell.contains("+'/upgrade/manifest.json'"))
    }

    @Test
    fun updateBridgeDoesNotCreateFakeInternalManifest() {
        assertTrue(bridge.contains("manifest_file_not_found"))
        assertFalse(bridge.contains("""put("versionName", "internal")"""))
        assertFalse(bridge.contains("""put("apkUrl", "fin-latest-debug.apk")"""))
    }

    @Test
    fun updateBridgeFailsExplicitlyWhenApkMissingAndVerifiesManifestIntegrity() {
        assertTrue(bridge.contains("apk_file_not_found"))
        assertFalse(bridge.contains("context.packageCodePath"))
        assertFalse(bridge.contains("package_code_path"))
        assertTrue(bridge.contains("expectedSize = obj.optLong(\"size\", -1L)"))
        assertTrue(bridge.contains("expectedSha256 = obj.optString(\"sha256\", \"\").trim().lowercase()"))
        assertTrue(bridge.contains("apk_size_mismatch"))
        assertTrue(bridge.contains("apk_sha256_mismatch"))
        assertTrue(bridge.contains("MessageDigest.getInstance(\"SHA-256\")"))
    }

    @Test
    fun installUpdateRequiresUnknownAppSourcePermissionExplicitly() {
        assertTrue(bridge.contains("canRequestPackageInstalls()"))
        assertTrue(bridge.contains("Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES"))
        assertTrue(bridge.contains("install_permission_required"))
        assertTrue(bridge.contains("settings_launched"))
    }

    @Test
    fun nativeReconnectIgnoresReplacedSocketCallbacks() {
        assertTrue(bridge.contains("private var nativeWsGeneration: Long = 0"))
        assertTrue(bridge.contains("nativeWsGeneration += 1"))
        assertTrue(bridge.contains("val generation = nativeWsGeneration"))
        assertTrue(bridge.contains("private fun isCurrent(): Boolean = generation == nativeWsGeneration"))
        assertTrue(bridge.contains("native_ws.stale_closed code=${'$'}code reason=${'$'}reason generation=${'$'}generation"))
        assertTrue(bridge.contains("native_ws.stale_failure generation=${'$'}generation detail=${'$'}detail"))
        assertTrue(bridge.contains("if (!isCurrent()) {\n                        appendConnectionEvent(\"native_ws.stale_closed"))
        assertTrue(bridge.contains("if (!isCurrent()) {\n                        appendConnectionEvent(\"native_ws.stale_failure"))
        assertFalse(bridge.contains("reason == \"replace_connection\""))
    }
}
