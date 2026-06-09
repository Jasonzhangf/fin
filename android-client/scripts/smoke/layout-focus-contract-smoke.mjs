#!/usr/bin/env node
import fs from 'fs'

const html = fs.readFileSync(new URL('../../app/src/main/assets/mobile-shell.html', import.meta.url), 'utf8')
const fails = []

function mustContains(snippet, name){
  if(!html.includes(snippet)) fails.push(`missing:${name}`)
}
function mustNotContains(snippet, name){
  if(html.includes(snippet)) fails.push(`forbidden:${name}`)
}

mustContains('.thread-history,.thread-live{display:flex;flex-direction:column;gap:10px;width:100%;max-width:100%;flex-shrink:0}', 'thread-history-live-layout')
mustContains('.app{width:100%;max-width:520px;margin:0 auto;height:100vh;min-height:100vh;display:flex;flex-direction:column;position:relative;overflow:hidden}', 'app-fixed-viewport-layout')
mustContains('.content{flex:1;min-height:0;padding:78px 0 var(--native-input-inset,96px);display:flex;align-items:flex-start;justify-content:flex-start;flex-direction:column;text-align:left', 'content-chat-layout')
mustContains('overflow-y:auto;overflow-x:hidden', 'content-overflow-axis-lock')
mustContains('function scrollToBottom(force){const content=document.getElementById(\'content\');if(!content)return;const doPin=!!force||(!inputFocused()&&!S.userScrolledUp&&!S.userTouchScrolling);', 'scroll-pin-policy')
mustContains('function setupScrollTracking(){const content=document.getElementById(\'content\');if(!content)return;', 'scroll-tracking-hook')
mustContains('const syncBottomState=()=>{const atBottom=content.scrollHeight-content.scrollTop-content.clientHeight<80;S.userScrolledUp=!atBottom}', 'scroll-bottom-threshold')
mustContains("content.addEventListener('touchstart',function(ev){S.userTouchScrolling=true;", 'touch-scroll-lock-start')
mustContains("const releaseTouchScroll=()=>{S.userTouchScrolling=false;syncBottomState()}", 'touch-scroll-lock-release')
mustContains('if(forceHistory){scrollToBottom(true);return;}scrollToBottom(false)', 'force-history-pin')
mustContains('body.sessions-open .bar,body.settings-open .bar{display:none}', 'settings-sessions-hide-native-composer')
mustContains('body.sessions-open .content,body.settings-open .content{padding-bottom:0}', 'settings-sessions-clear-native-inset')
mustContains("function openSettings(){document.body.classList.add('settings-open');applyNativeChromeMode('settings');", 'settings-native-chrome-mode')
mustContains("if(id==='settingsPanel'){document.body.classList.remove('settings-open');applyNativeChromeMode('chat')}", 'settings-native-chrome-restore')
mustContains('async function runUpdateFlow()', 'single-semantic-update-flow')
mustContains('function parseBridgeResult(raw)', 'semantic-bridge-result-parser')
mustContains('function updateStatus(text,detail,error)', 'semantic-update-status')
mustContains('function renderErrorBubble(text)', 'turn-error-bubble')
mustContains("const assistant=answer?renderAssistantBubble(answer):(err?renderErrorBubble(err):renderStatusBubble('未返回内容'))", 'turn-error-answer-selector')
mustContains('function pushLocalErrorTurn(id,payload,error)', 'local-send-error-turn')
mustContains('dispatchUserPayload(payload,{silentLocalError:true})', 'silent-autosend-local-error')
mustContains('function foregroundResume()', 'foreground-resume-entry')
mustContains("connectWs('foreground')", 'foreground-resume-reconnect')
mustContains('function progressItemFromPhase(m)', 'progress-term-projection')
mustContains("label:'provider.call',title:'Provider Call'", 'provider-progress-term')
mustContains("label:'tool.dispatch',title:'Tool Dispatch'", 'tool-progress-term')
mustNotContains('id="updateManifestInput"', 'raw-update-manifest-input')
mustNotContains('id="btnCheckUpdate"', 'raw-check-update-button')
mustNotContains('id="btnDownloadUpdate"', 'raw-download-update-button')
mustNotContains('id="btnInstallUpdate"', 'raw-install-update-button')
mustNotContains("textContent=String(r||'ok')", 'raw-bridge-json-render')
mustNotContains('id="debugToggle"', 'raw-debug-toggle')
mustNotContains('function scrollToBottom(){if(inputFocused())return;', 'legacy-scroll-early-return')
mustNotContains('if(!inputFocused())scrollToBottom(false)', 'inset-update-no-forced-pin')
mustNotContains('window.scrollTo(0,document.body.scrollHeight)', 'forbidden-window-scroll-pin')

if(fails.length){
  console.error('LAYOUT_FOCUS_CONTRACT_FAIL')
  for(const f of fails) console.error('-', f)
  process.exit(1)
}
console.log('LAYOUT_FOCUS_CONTRACT_OK')
