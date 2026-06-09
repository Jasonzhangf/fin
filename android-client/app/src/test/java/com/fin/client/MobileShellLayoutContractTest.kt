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
        assertTrue(shell.contains(".content{flex:1;min-height:0;padding:78px 0 var(--native-input-inset,96px);display:flex;align-items:flex-start;justify-content:flex-start"))
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
        assertTrue(shell.contains(".app{width:100%;max-width:520px;margin:0 auto;height:100vh;min-height:100vh;display:flex;flex-direction:column;position:relative;overflow:hidden}"))
        assertTrue(shell.contains(".content{flex:1;min-height:0;padding:78px 0 var(--native-input-inset,96px);display:flex;align-items:flex-start;justify-content:flex-start;flex-direction:column;text-align:left"))
        assertTrue(shell.contains(".thread-history,.thread-live{display:flex;flex-direction:column;gap:10px;width:100%;max-width:100%;flex-shrink:0}"))
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
    fun turnErrorsAndLocalSendFailuresRenderInConversation() {
        assertTrue(shell.contains("function turnErrorText(t)"))
        assertTrue(shell.contains("function renderErrorBubble(text)"))
        assertTrue(shell.contains(".bubble-error{background:rgba(180,35,24,.12);border-color:var(--danger);color:var(--text);border-bottom-left-radius:8px}"))
        assertTrue(shell.contains(".message-text.error{color:var(--danger);font-weight:700}"))
        assertTrue(shell.contains("const assistant=answer?renderAssistantBubble(answer):(err?renderErrorBubble(err):renderStatusBubble('未返回内容'))"))
        assertTrue(shell.contains("function pushLocalErrorTurn(id,payload,error)"))
        assertTrue(shell.contains("const showLocalError=!opts.silentLocalError"))
        assertTrue(shell.contains("if(showLocalError)pushLocalErrorTurn('',payload,'未连接到 Daemon')"))
        assertTrue(shell.contains("if(showLocalError)pushLocalErrorTurn('',payload,'暂无可用会话')"))
        assertTrue(shell.contains("if(showLocalError)pushLocalErrorTurn(id,payload,'发送失败：连接不可用')"))
        assertTrue(shell.contains("dispatchUserPayload(payload,{silentLocalError:true})"))
        assertFalse(shell.contains("if(S.conn!=='healthy'){toast('未连接');return false;}"))
        assertFalse(shell.contains("if(!S.currentSessionId){toast('暂无可用会话');return false;}"))
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
        assertTrue(shell.contains("syncUpdateManifestFromConfig();"))
        assertFalse(shell.contains("+'/upgrade/manifest.js'"))
        assertFalse(shell.contains("+'/upgrade/manifest.json'"))
    }

    @Test
    fun updateUiRunsSingleSemanticFlowWithoutRawBridgeJson() {
        assertTrue(shell.contains("""id="btnRunUpdate""""))
        assertTrue(shell.contains("""onclick="runUpdateFlow()""""))
        assertTrue(shell.contains("async function runUpdateFlow()"))
        assertTrue(shell.contains("function parseBridgeResult(raw)"))
        assertTrue(shell.contains("function updateStatus(text,detail,error)"))
        assertTrue(shell.contains("function updateErrorMessage(code)"))
        assertTrue(shell.contains("需要允许安装未知应用"))
        assertTrue(shell.contains("升级包校验失败"))
        assertTrue(shell.contains("已打开系统安装器"))
        assertFalse(shell.contains("""id="updateManifestInput""""))
        assertFalse(shell.contains("""id="btnCheckUpdate""""))
        assertFalse(shell.contains("""id="btnDownloadUpdate""""))
        assertFalse(shell.contains("""id="btnInstallUpdate""""))
        assertFalse(shell.contains("function checkUpdate()"))
        assertFalse(shell.contains("function downloadUpdate()"))
        assertFalse(shell.contains("function installUpdate()"))
        assertFalse(shell.contains("textContent=String(r||'ok')"))
        assertFalse(shell.contains("""id="debugToggle""""))
        assertFalse(shell.contains("function toggleDebugMode()"))
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
    fun nativeChromeModeHidesInputForSettingsAndSessionsPanels() {
        assertTrue(activity.contains("""val chromePanelOpen = mode == "sessions" || mode == "settings""""))
        assertTrue(activity.contains("nativeInputBar?.visibility = if (chromePanelOpen) View.GONE else View.VISIBLE"))
        assertTrue(shell.contains("body.sessions-open .bar,body.settings-open .bar{display:none}"))
        assertTrue(shell.contains("body.sessions-open .content,body.settings-open .content{padding-bottom:0}"))
        assertTrue(shell.contains("function openSettings(){document.body.classList.add('settings-open');applyNativeChromeMode('settings');"))
        assertTrue(shell.contains("if(id==='settingsPanel'){document.body.classList.remove('settings-open');applyNativeChromeMode('chat')}"))
    }

    @Test
    fun nativeReconnectForwardsStaleTurnResultsOnly() {
        assertTrue(bridge.contains("private var nativeWsGeneration: Long = 0"))
        assertTrue(bridge.contains("nativeWsGeneration += 1"))
        assertTrue(bridge.contains("val generation = nativeWsGeneration"))
        assertTrue(bridge.contains("private fun isCurrent(): Boolean = generation == nativeWsGeneration"))
        assertTrue(bridge.contains("native_ws.stale_closed code=${'$'}code reason=${'$'}reason generation=${'$'}generation"))
        assertTrue(bridge.contains("native_ws.stale_failure generation=${'$'}generation detail=${'$'}detail"))
        assertTrue(bridge.contains("private fun isTurnResultFrame(text: String): Boolean"))
        assertTrue(bridge.contains("type == \"input.accepted\""))
        assertTrue(bridge.contains("type == \"turn.completed\""))
        assertTrue(bridge.contains("type == \"turn.rendered\""))
        assertTrue(bridge.contains("native_ws.forward_stale_turn generation=${'$'}generation"))
        assertTrue(bridge.contains("if (isTurnResultFrame(text)) {\n                            appendConnectionEvent(\"native_ws.forward_stale_turn"))
        assertTrue(bridge.contains("if (!isCurrent()) {\n                        appendConnectionEvent(\"native_ws.stale_closed"))
        assertTrue(bridge.contains("if (!isCurrent()) {\n                        appendConnectionEvent(\"native_ws.stale_failure"))
        assertFalse(bridge.contains("reason == \"replace_connection\""))
    }

    @Test
    fun foregroundResumeMustReconnectAndRehydrateSessionTruth() {
        assertTrue(shell.contains("function foregroundResume(){log('app.foreground_resume');try{CONFIG.load()}catch(_){}loadProfiles();renderTurns(true);connectWs('foreground')}"))
        assertTrue(activity.contains("if(typeof foregroundResume === 'function') foregroundResume(); else if(typeof CONFIG !== 'undefined') CONFIG.load();"))
        assertTrue(activity.indexOf("webView.webViewClient = object : WebViewClient()") < activity.indexOf("webView.loadUrl(\"file:///android_asset/mobile-shell.html\")"))
        assertFalse(activity.contains("webView.webViewClient = WebViewClient()"))
    }

    @Test
    fun finAutoSendIntentIsConsumedOnce() {
        assertTrue(activity.contains("intent?.getStringExtra(\"finAutoSend\")?.takeIf { it.isNotBlank() }?.let { payload ->"))
        assertTrue(activity.contains("intent?.removeExtra(\"finAutoSend\")"))
        assertTrue(activity.indexOf("intent?.removeExtra(\"finAutoSend\")") < activity.indexOf("device_e2e_intent_autosend len=${'$'}{payload.length}"))
    }

    @Test
    fun mobileShellLogsTurnReceiveAndRenderProgressSemantically() {
        assertTrue(shell.contains("function logWsReceive(m)"))
        assertTrue(shell.contains("function updatePending(id,state,phase)"))
        assertTrue(shell.contains("function clearPending(id,reason)"))
        assertTrue(shell.contains("function logOnce(key,prefix,kv)"))
        assertTrue(shell.contains("logWsReceive(m);"))
        assertTrue(shell.contains("'ws_recv'"))
        assertTrue(shell.contains("'ui_pending_add'"))
        assertTrue(shell.contains("'ui_pending_update'"))
        assertTrue(shell.contains("'ui_pending_clear'"))
        assertTrue(shell.contains("'ui_live_render'"))
        assertTrue(shell.contains("'ui_item_upsert'"))
        assertTrue(shell.contains("'ui_item_render'"))
        assertTrue(shell.contains("'ui_turn_completed'"))
        assertTrue(shell.contains("'ui_turn_rendered'"))
        assertTrue(shell.contains("const visibleLive=visibleTimelineItems(live)"))
        assertTrue(shell.contains("function progressItemFromPhase(m)"))
        assertTrue(shell.contains("label:'provider.call',title:'Provider Call'"))
        assertTrue(shell.contains("label:'tool.dispatch',title:'Tool Dispatch'"))
        assertTrue(shell.contains("function firstText(){for(const v of arguments)"))
        assertTrue(shell.contains("a:firstText(t.assistant_response,t.assistant_visible_output,t.answer)"))
        assertTrue(shell.contains("if(id)upsertProgressItem(id,m); renderTurns(false); return;"))
        assertTrue(shell.contains("function mergeRenderedItems(finalItems,liveItems)"))
        assertFalse(shell.contains("function isInternalItem"))
        assertFalse(shell.contains("label==='provider.call'"))
        assertFalse(shell.contains("label==='reasoning.stop'"))
        assertFalse(shell.contains("target_kind||'')==='provider'"))
        assertFalse(shell.contains("target_kind||'')==='reasoning_closure'"))
        assertTrue(shell.contains("clearPending(id,'turn.rendered')"))
        assertTrue(shell.contains("clearPending(id,'send_failed');if(showLocalError)pushLocalErrorTurn(id,payload,'发送失败：连接不可用')"))
        assertFalse(shell.contains("log(JSON.stringify(m))"))
        assertFalse(shell.contains("appendConnectionEvent(JSON.stringify"))
    }
}
