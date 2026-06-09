#!/usr/bin/env node
const S={conn:'idle',turns:[],pendingById:{},traceByClientId:{},itemByClientId:{},turnStateByClientId:{},runtimeHealth:'schema_error:runtime.health',providerHealth:'schema_error:provider.health',cache:'',activityCards:null,expandedAgentCards:{},sessions:[],selectedSessions:new Set(),pendingNewSession:false,renderedTurnCount:0,uiRenderLogKeys:new Set()};
const nativeSent=[];
const semanticLogs=[];
let activeProvider='';
let activeModel='';
function log(m){semanticLogs.push(String(m))}
function logKv(prefix,kv){const parts=[prefix];Object.keys(kv||{}).forEach(k=>{const v=String(kv[k]??'').replace(/\s+/g,'_');parts.push(k+'='+(v||'-'))});log(parts.join(' '))}
function eventClientId(m){return String((m&&m.client_message_id)||'')}
function eventItemId(m){return String((m&&m.item_id)||(m&&m.tool_call_id)||'')}
function eventPhase(m){return String((m&&m.phase)||(m&&m.serverPhase)||(m&&m.state)||(m&&m.status)||'')}
function isTurnLifecycleType(type){return type==='input.accepted'||type==='turn.started'||type==='turn.progress'||type==='turn.completed'||type==='turn.rendered'||type==='turn.trace_event'||type.startsWith('turn.item.')}
function logWsReceive(m){const type=String(m&&m.type||'');if(!isTurnLifecycleType(type))return;logKv('ws_recv',{type,client:eventClientId(m),phase:eventPhase(m),item:eventItemId(m),status:String(m&&m.status||'')})}
function pendingPhase(id){const p=id?S.pendingById[id]:null;return p?String(p.serverPhase||p.state||''):'missing'}
function logPendingUpdate(id){if(id)logKv('ui_pending_update',{client:id,phase:pendingPhase(id)})}
function updatePending(id,state,phase){if(!id)return false;const p=S.pendingById[id];if(!p){logKv('ui_pending_missing',{client:id,phase:phase||state||'unknown'});return false;}p.state=state||p.state;p.serverPhase=phase||p.serverPhase||p.state;logPendingUpdate(id);return true}
function clearPending(id,reason){if(!id)return;if(S.pendingById[id]){delete S.pendingById[id];logKv('ui_pending_clear',{client:id,reason:reason||'unknown'})}}
function logOnce(key,prefix,kv){if(!key||S.uiRenderLogKeys.has(key))return;S.uiRenderLogKeys.add(key);logKv(prefix,kv)}
function addPending(id,text,ts){S.pendingById[id]={id,text,state:'sending',serverPhase:'sending',ts:ts||Date.now()};logKv('ui_pending_add',{client:id,phase:'sending'})}
function schema(v,field){const s=(v&&v[field]!=null)?String(v[field]).trim():'';return s?s:'schema_error:'+field}
function setConn(s){S.conn=s}
function setRuntimeState(s){S.runtimeHealth=s}
function setProviderState(s){S.providerHealth=s}
function persistProviderConfigCache(v){S.cache=JSON.stringify(v||{})}
function renderActiveConfig(m){const profiles=Array.isArray(m&&m.profiles)?m.profiles:[];const def=String((m&&m.default_profile)||'').trim();let active=profiles.find(p=>p&&p.active);if(!active&&def)active=profiles.find(p=>String((p&&p.profile_name)||p?.provider||'')===def);if(active){activeProvider=String(active.provider||active.profile_name||def||'');activeModel=String(active.model||'')}else{activeProvider=def;activeModel='schema_error:model'}}
function itemFromRecord(rec){return {item_id:schema(rec,'tool_call_id'),label:schema(rec,'tool_name'),title:schema(rec,'title'),purpose:schema(rec,'purpose'),status:schema(rec,'status'),duration_ms:rec?.duration_ms??0,output_summary:rec?.output_summary||'',error_summary:rec?.error_summary||'',item_kind:rec?.tool_kind||'schema_error:item_kind',target_kind:rec?.target_kind||''}}
function normalizeToolRecord(rec){return itemFromRecord(rec||{})}
function normalizeErrorRecord(e){const item=itemFromRecord(e||{});item.status='failed';item.error_summary=schema(e||{},'error_summary');return item}
function toTurn(t){return {turn_id:t.turn_id||'',client_message_id:t.client_message_id||'',u:t.user_input||'',a:t.assistant_response||'',control:t.control_feedback_summary||'',tool:t.tool_execution_summary||'',closure:t.closure_stop_source||'',items:(t.tool_execution_records||[]).map(normalizeToolRecord).concat((t.error_records||[]).map(normalizeErrorRecord))}}
function bridge(){return {nativeWsSend:(payload)=>{nativeSent.push(JSON.parse(payload));return 'ok'}}}
function sendWs(payload){const text=typeof payload==='string'?payload:JSON.stringify(payload);const b=bridge();if(b&&b.nativeWsSend)return b.nativeWsSend(text)==='ok';return false}
function normalizeSemanticAction(a){return {item_id:a.tool_call_id||a.item_id||a.summary||'agent-action',label:a.tool_name||a.verb||'agent.action',title:a.object_label||a.object_kind||a.summary||'agent action',purpose:a.summary||'',status:a.status||'completed',output_summary:a.detail||a.summary||'',error_summary:a.failure_detail||'',item_kind:a.category||'agent_activity',target_kind:a.object_kind||''}}
function projectAgentCards(){const snap=S.activityCards||{};return (snap.source_cards||[]).filter(c=>String(c.source_kind||'').includes('project')||String(c.source_id||'').includes('project'))}
function shortAgentName(c){let raw=String(c.display_name||c.title||c.agent_name||c.source_id||'').replace(/^Peer\s+/,'').replace(/^peer[-_]/,'').trim();if(!raw||raw.match(/^project[-_]?agent$/i)||raw==='agent'){const id=String(c.source_id||'');raw=id.includes('.')?id.split('.').pop():id.replace(/^peer-project-agent-/,'').replace(/^project[-_]/,'')}if(!raw)raw='agent';return raw.length>18?raw.slice(0,16)+'…':raw}
function agentStatusKind(c){const text=[c.state,c.summary,c.current_activity,c.presence_state,c.connectivity_state,c.binding_state,c.current_phase].map(x=>String(x||'').toLowerCase()).join(' ');if(text.match(/error|failed|unreachable|problem|stale/))return 'problem';if(text.match(/running|busy|working|executing|assigned|reasoning|inference|provider_wait|推理/))return 'busy';if(text.match(/offline|disconnected|missing|unknown/))return 'offline';return 'idle'}
function agentStatusLabel(kind){return kind==='problem'?'有问题':kind==='busy'?'忙':kind==='offline'?'离线':'闲'}
function renderAgentCards(){return projectAgentCards().map(c=>{const kind=agentStatusKind(c);return {name:shortAgentName(c),kind,label:agentStatusLabel(kind)}})}
function agentCardActions(c){const recent=Array.isArray(c.recent_actions)?c.recent_actions.map(normalizeSemanticAction):[];if(recent.length>0)return recent;const summary=String(c.current_activity||c.summary||'').trim();return summary?[{item_id:String(c.source_id||summary),label:'agent.activity',title:summary,purpose:summary,status:agentStatusKind(c)==='busy'?'running':'completed',output_summary:summary,error_summary:'',item_kind:'agent_activity',target_kind:'project_agent'}]:[]}
function toggleAgentCard(agentId){if(!agentId)return;S.expandedAgentCards[agentId]=!S.expandedAgentCards[agentId]}
function renderPinnedAgentCards(){return projectAgentCards().map(c=>{const id=String(c.source_id||shortAgentName(c));const expanded=!!S.expandedAgentCards[id];return {id,name:shortAgentName(c),kind:agentStatusKind(c),expanded,summary:String(c.current_activity||c.summary||''),waiting:String(c.waiting_detail||''),failure:String(c.failure_detail||''),actions:agentCardActions(c)}})}
function upsertItem(id,m){if(!id)return;const by=S.itemByClientId[id]||(S.itemByClientId[id]={});const itemId=schema(m,'item_id');const prev=by[itemId]||{};const item=Object.assign({},prev,m,{item_id:itemId,label:schema(m,'label'),title:schema(m,'title'),purpose:schema(m,'purpose'),status:schema(m,'status')});by[itemId]=item;logKv('ui_item_upsert',{client:id,item:itemId,status:item.status,label:item.label})}
function consumeItems(id){const by=S.itemByClientId[id]||{};delete S.itemByClientId[id];return Object.values(by)}
function bindSession(id){S.currentSessionId=id; sendWs({type:'session.bind',session_id:id})}
function createNewSession(){S.pendingNewSession=true;const ok=sendWs({type:'session.command',command:'/new'});if(!ok)S.pendingNewSession=false;return ok}
function clearCurrentSessionView(){S.currentSessionId=null;S.turns=[];S.pendingById={};S.itemByClientId={};S.traceByClientId={};S.turnStateByClientId={};S.uiRenderLogKeys=new Set();S.historyLoaded=false;S.renderedTurnCount=0}
function newestSessionId(){return S.sessions.length?String(S.sessions[0].session_id||''):''}
function reconcileCurrentSession(){const ids=S.sessions.map(s=>String(s.session_id||''));if(S.pendingNewSession){const latest=newestSessionId();if(latest){S.pendingNewSession=false;bindSession(latest);return;}}if(S.currentSessionId&&ids.includes(String(S.currentSessionId)))return;if(S.currentSessionId)clearCurrentSessionView();if(!S.currentSessionId&&S.sessions.length>0)bindSession(S.sessions[0].session_id)}
function chooseSession(id){S.selectedSessions.clear();S.selectedSessions.add(id);bindSession(id)}
function selectedSessionIds(){return Array.from(S.selectedSessions).filter(id=>S.sessions.some(s=>s.session_id===id))}
function toggleSessionSelection(id){if(S.selectedSessions.has(id))S.selectedSessions.delete(id);else S.selectedSessions.add(id)}
function archiveSelectedSessions(){const ids=selectedSessionIds();if(ids.length)sendWs({type:'session.archive',session_ids:ids})}
function deleteSelectedSessions(){const ids=selectedSessionIds();if(ids.length){sendWs({type:'session.delete',session_ids:ids});if(S.currentSessionId&&ids.includes(String(S.currentSessionId)))clearCurrentSessionView();S.selectedSessions.clear()}}
function renameSelectedSession(title){const ids=selectedSessionIds();if(ids.length===1&&title)sendWs({type:'session.rename',session_id:ids[0],title})}
function onWs(m){
  logWsReceive(m)
  switch(m.type){
    case 'session.list': S.sessions=m.sessions||[]; reconcileCurrentSession(); return;
    case 'session.operation.ok': if(m.operation==='delete'&&Array.isArray(m.session_ids)&&S.currentSessionId&&m.session_ids.map(String).includes(String(S.currentSessionId)))clearCurrentSessionView(); return;
    case 'session.operation.failed': return;
    case 'config.snapshot': persistProviderConfigCache(m); renderActiveConfig(m); return;
    case 'input.accepted': {const id=m.client_message_id||''; updatePending(id,'accepted','queued'); return;}
    case 'runtime.health': setRuntimeState(schema(m,'status')); return;
    case 'provider.health': setProviderState(schema(m,'status')); return;
    case 'activity.cards.snapshot': {S.activityCards=m.snapshot||m.cards||m; return;}
    case 'protocol.error': return;
    case 'turn.started': {const id=m.client_message_id||''; if(id){S.turnStateByClientId[id]=m;updatePending(id,'waiting','running')} setRuntimeState('running'); return;}
    case 'turn.progress': {const id=m.client_message_id||''; updatePending(id,'waiting',m.phase||'inference_waiting'); return;}
    case 'turn.item.started':
    case 'turn.item.delta':
    case 'turn.item.completed':
    case 'turn.item.failed': {const id=m.client_message_id||''; upsertItem(id,m); return;}
    case 'turn.completed': {setRuntimeState(m.status==='failed'?'failed':'ok'); logKv('ui_turn_completed',{client:m.client_message_id||'',status:String(m.status||'')}); return;}
    case 'turn.trace_event': {const id=m.client_message_id||''; if(id){(S.traceByClientId[id]??=[]).push(m)} return;}
    case 'turn.rendered': {const id=m.client_message_id||''; clearPending(id,'turn.rendered'); const t=toTurn(m); if(id){t.items=t.items.concat(consumeItems(id)); delete S.turnStateByClientId[id];} if(id&&S.traceByClientId[id]){t.control=(t.control||'')+';trace_events='+String(S.traceByClientId[id].length); delete S.traceByClientId[id];} S.turns.push(t); return;}
    default: return;
  }
}
function itemLabelOf(r){return String((r&&r.label)||(r&&r.tool_name)||'').trim()}
function itemKindOf(r){return String((r&&r.item_kind)||(r&&r.tool_kind)||'').trim()}
function isInternalItem(r){const label=itemLabelOf(r);const kind=itemKindOf(r);if(label==='provider.call'||label==='reasoning.stop'||label==='session.list')return true;if(kind==='framework_tool')return true;if(String(r&&r.target_kind||'')==='provider')return true;if(String(r&&r.target_kind||'')==='reasoning_closure')return true;return false}
function visibleTimelineItems(items){const by={};for(const r of (items||[])){const key=String(r&&r.item_id||r&&r.tool_call_id||JSON.stringify(r));by[key]=Object.assign({},by[key]||{},r)}return Object.values(by).filter(r=>{const st=String(r&&r.status||'');const err=String(r&&r.error_summary||'').trim();if(err||st==='failed')return true;return !isInternalItem(r)})}
function renderToolTimelineFromItems(items,live){const rows=[];for(const r of visibleTimelineItems(items)){const st=schema(r,'status');const err=String(r.error_summary||'').trim();const sem={title:schema(r,'title'),label:schema(r,'label'),detail:String(r.output_summary||'').trim()||schema(r,'purpose'),status:schema(r,'status')};const dur=(r.duration_ms!=null&&st!=='running')?` · ${r.duration_ms}ms`:'';const liveMark=(live&&st==='running')?' …':'';rows.push(`<div class='tl-item'><b>${sem.title}</b> ${sem.label} ${sem.status}${dur}${liveMark}${err?` ${err}`:''}<span>${sem.detail}</span></div>`)}if(rows.length===0)return '';return `<div class='timeline'><div class='tl-head'>${live?'工具执行（实时）':'工具执行'}</div>${rows.join('')}</div>`}
function bubbleRow(kind,body,extraClass){return `<div class='msg-row ${kind}'><div class='bubble ${extraClass||''}'>${body}</div></div>`}
function renderUserBubble(text){return bubbleRow('user',`<div class='message-text user'>${text||''}</div>`,'bubble-user')}
function renderAssistantBubble(text){return bubbleRow('assistant',`<div class='message-text'>${text||''}</div>`,'bubble-assistant')}
function renderStatusBubble(text){return bubbleRow('status',`<div class='status'>${text}</div>`,'bubble-status')}
function renderTurnThread(t){const timeline=renderToolTimelineFromItems(t.items||[],false);return `<div class='chat-group' data-turn-id='${t.turn_id||t.client_message_id||''}' data-render-style='chat-thread'>${renderUserBubble(t.u)}${renderAssistantBubble(t.a)}${timeline}</div>`}
function renderLiveThread(p,items){items=items||[];const visibleLive=visibleTimelineItems(items);logOnce('live:'+p.id+':'+String(p.serverPhase||p.state||'')+':'+String(visibleLive.length),'ui_live_render',{client:p.id,phase:String(p.serverPhase||p.state||''),items:visibleLive.length});visibleLive.forEach(item=>logOnce('live-item:'+p.id+':'+String(item.item_id||'')+':'+String(item.status||''),'ui_item_render',{client:p.id,item:String(item.item_id||''),status:String(item.status||''),label:String(item.label||'')}));const phaseMap={sending:'发送中',accepted:'已送达服务器',waiting:'等待推理结果',inference_waiting:'推理中',provider_wait:'等待模型响应',tool_wait:'工具执行中',queued:'排队中',running:'处理中'};const timeline=renderToolTimelineFromItems(items,true);return `<div class='chat-group pending' data-client-message-id='${p.id||''}' data-render-style='chat-thread'>${renderUserBubble(p.text||'')}${renderStatusBubble((phaseMap[p.serverPhase]||phaseMap[p.state]||'处理中')+' …')}${timeline}</div>`}
function renderConversationThread(force){const pending=Object.values(S.pendingById).sort((a,b)=>(a.ts||0)-(b.ts||0));pending.forEach(p=>renderLiveThread(p,Object.values(S.itemByClientId[p.id]||{})));logKv('ui_turn_rendered',{turns:S.turns.length,pending:pending.length,mode:force?'force':'append'})}
function assert(cond,msg){if(!cond) throw new Error(msg)}
function assertItem(item){
  for(const f of ['item_id','label','title','purpose','status']) assert(item[f] && !String(item[f]).startsWith('schema_error:'), `bad ${f}: ${JSON.stringify(item)}`)
  assert(!['tool','unknown',''].includes(String(item.label).trim()), 'non-informative label')
  assert(!['tool','unknown',''].includes(String(item.title).trim()), 'non-informative title')
}
function assertLogContains(fragment){assert(semanticLogs.some(x=>x.includes(fragment)), `missing semantic log: ${fragment}\n${semanticLogs.join('\n')}`)}
function assertLogOrder(fragments){
  let pos=-1
  for(const f of fragments){
    const next=semanticLogs.findIndex((x,i)=>i>pos&&x.includes(f))
    assert(next>pos, `semantic log out of order or missing: ${f}\n${semanticLogs.join('\n')}`)
    pos=next
  }
}
onWs({type:'config.snapshot',default_profile:'minimax',profiles:[{profile_name:'minimax',provider:'minimax',protocol:'anthropic-wire',model:'MiniMax-M3',active:true}],active_thinking_effort:null})
addPending('m1','hi',1)
renderConversationThread(false)
onWs({type:'input.accepted',client_message_id:'m1'})
renderConversationThread(false)
onWs({type:'runtime.health',status:'available'})
onWs({type:'provider.health',status:'available',provider:'minimax',model:'MiniMax-M3'})
onWs({type:'turn.started',client_message_id:'m1',session_id:'s1',turn_id:'t1'})
renderConversationThread(false)
onWs({type:'turn.progress',client_message_id:'m1',phase:'tool_wait'})
renderConversationThread(false)
onWs({type:'turn.item.started',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i1',item_kind:'provider',label:'provider.call',title:'Provider Call',purpose:'dispatch compiled prompt',status:'running',started_at:'now'})
renderConversationThread(false)
onWs({type:'turn.item.completed',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i1',item_kind:'provider',label:'provider.call',title:'Provider Call',purpose:'dispatch compiled prompt',status:'completed',duration_ms:12,output_summary:'ok',error_summary:null})
renderConversationThread(false)
onWs({type:'turn.item.started',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i2',item_kind:'exec',label:'shell.exec',title:'Execute shell',purpose:'run local command',status:'running',started_at:'now'})
renderConversationThread(false)
onWs({type:'turn.item.failed',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i2',item_kind:'exec',label:'shell.exec',title:'Execute shell',purpose:'run local command',status:'failed',duration_ms:3,error_summary:'command not found'})
renderConversationThread(false)
onWs({type:'turn.trace_event',client_message_id:'m1',detail:'trace'})
onWs({type:'turn.completed',client_message_id:'m1',session_id:'s1',turn_id:'t1',status:'completed'})
onWs({type:'turn.rendered',client_message_id:'m1',user_input:'hi',assistant_response:'ok'})
renderConversationThread(true)
assertLogOrder([
  'ui_pending_add client=m1 phase=sending',
  'ws_recv type=input.accepted client=m1',
  'ui_pending_update client=m1 phase=queued',
  'ws_recv type=turn.started client=m1',
  'ui_pending_update client=m1 phase=running',
  'ws_recv type=turn.progress client=m1 phase=tool_wait',
  'ui_pending_update client=m1 phase=tool_wait',
  'ws_recv type=turn.item.started client=m1 phase=running item=i2 status=running',
  'ui_item_upsert client=m1 item=i2 status=running label=shell.exec',
  'ui_item_render client=m1 item=i2 status=running label=shell.exec',
  'ws_recv type=turn.completed client=m1 phase=completed',
  'ui_turn_completed client=m1 status=completed',
  'ws_recv type=turn.rendered client=m1',
  'ui_pending_clear client=m1 reason=turn.rendered',
  'ui_turn_rendered turns=1 pending=0 mode=force',
])
assertLogContains('ui_live_render client=m1 phase=tool_wait')
const frozenTurnJson=JSON.stringify(S.turns[0])
addPending('m2','next',2)
onWs({type:'input.accepted',client_message_id:'m2'})
onWs({type:'turn.item.started',client_message_id:'m2',session_id:'s1',turn_id:'t2',item_id:'i9',item_kind:'exec',label:'shell.exec',title:'Execute shell',purpose:'new task',status:'running',started_at:'now'})
assert(JSON.stringify(S.turns[0])===frozenTurnJson,'historical finalized turn must stay immutable when new live item arrives')
onWs({type:'activity.cards.snapshot',snapshot:{source_cards:[{source_id:'local.project-fin',source_kind:'project_agent',title:'builder',agent_name:'builder',state:'running',recent_actions:[{tool_call_id:'tool-1',tool_name:'project.tool',category:'tools',summary:'named agent consumed delegated turn',status:'completed'}]}]}})
const renderedTurnHtml=renderTurnThread(S.turns[0])
assert(renderedTurnHtml.includes("data-render-style='chat-thread'"),'final turn must render as chat thread')
assert(renderedTurnHtml.includes('bubble-user'),'final turn must include user bubble')
assert(renderedTurnHtml.includes('bubble-assistant'),'final turn must include assistant bubble')
assert(!renderedTurnHtml.includes('你：'),'final turn must not use legacy Q&A label chrome')
const renderedLiveHtml=renderLiveThread({id:'m-live',text:'live ask',state:'waiting',serverPhase:'tool_wait'},[{item_id:'tl-1',label:'shell.exec',title:'Execute shell',purpose:'run',status:'running'}])
assert(renderedLiveHtml.includes('bubble-status'),'live thread must render status bubble')
assert(renderedLiveHtml.includes('工具执行（实时）'),'live thread must keep live timeline heading')
onWs({type:'session.list',sessions:[{session_id:'s1',title:'任务一'},{session_id:'s2',title:'任务二',archived:true}]})
assert(S.currentSessionId==='s1','session list must bind first session')
onWs({type:'session.operation.ok',operation:'delete',session_ids:['s1']})
assert(S.currentSessionId===null&&S.turns.length===0,'deleting current session must clear current view')
bindSession('session-1')
createNewSession()
onWs({type:'session.list',sessions:[{session_id:'s3',title:'新任务'},{session_id:'s2',title:'任务二'}]})
assert(S.currentSessionId==='s3','new session must auto-bind newest listed session')
chooseSession('s2')
assert(S.currentSessionId==='s2','choosing a session must bind input to that session')
onWs({type:'session.list',sessions:[{session_id:'s1',title:'任务一'},{session_id:'s2',title:'任务二',archived:true}]})
S.selectedSessions.clear()
toggleSessionSelection('s1')
renameSelectedSession('新任务名')
archiveSelectedSessions()
toggleSessionSelection('s2')
deleteSelectedSessions()
assert(S.turns.length===0,'deleted session view must stay cleared')
const visible=visibleTimelineItems([{item_id:'p',label:'provider.call',title:'Provider Call',purpose:'dispatch',status:'completed',item_kind:'provider',target_kind:'provider',output_summary:'cache_hit_rate=unknown'},{item_id:'x',label:'shell.exec',title:'Execute shell',purpose:'run',status:'failed',error_summary:'command not found'}])
assert(visible.length===1&&visible[0].label==='shell.exec','provider call must be hidden from visible timeline')
visible.forEach(assertItem)
assert(visible.some(i=>i.status==='failed' && i.error_summary==='command not found'),'failed item error missing')
assert(S.cache.includes('default_profile'),'config snapshot not persisted')
assert(S.cache.includes('minimax')&&S.cache.includes('MiniMax-M3'),'config snapshot must preserve minimax provider/model')
assert(activeProvider==='minimax','active provider render must use runtime config snapshot')
assert(activeModel==='MiniMax-M3','active model render must use runtime config snapshot')
assert(S.runtimeHealth==='ok','runtime health not separated')
assert(S.providerHealth==='available','provider health not separated')
let cards=renderAgentCards()
assert(cards.length===1,'project agent status button missing')
assert(cards[0].kind==='busy'&&cards[0].label==='忙','running project agent must render blue busy status')
assert(cards[0].name==='builder','agent status must use persisted agent name')
assert(!cards[0].name.includes('project_agent'),'agent status must not expose generic project_agent kind')
toggleAgentCard('local.project-fin')
let pins=renderPinnedAgentCards()
assert(pins.length===1,'project agent pinned card missing')
assert(pins[0].expanded===true,'project agent pinned card must toggle expanded')
assert(pins[0].actions.length===1 && pins[0].actions[0].label==='project.tool','project agent pinned card must include recent delegated action')
onWs({type:'activity.cards.snapshot',snapshot:{source_cards:[{source_id:'local.project-fin',source_kind:'project_agent',title:'agent',state:'ready'}]}})
cards=renderAgentCards()
assert(cards[0].name==='project-fin','legacy nameless agent must derive stable non-generic name')
onWs({type:'activity.cards.snapshot',snapshot:{source_cards:[{source_id:'local.project-fin',source_kind:'project_agent',title:'builder',state:'ready',current_phase:'provider_wait'}]}})
cards=renderAgentCards()
assert(cards[0].kind==='busy','reasoning/provider wait agent must render busy')
onWs({type:'activity.cards.snapshot',snapshot:{source_cards:[{source_id:'remote.builder',source_kind:'project_agent',title:'10.0.0.8.builder',agent_name:'builder',display_name:'10.0.0.8.builder',state:'ready'}]}})
cards=renderAgentCards()
assert(cards[0].name==='10.0.0.8.builder','remote agent status must show device/ip prefix')
pins=renderPinnedAgentCards()
assert(pins[0].name==='10.0.0.8.builder','remote pinned agent must keep device/ip prefix')
onWs({type:'activity.cards.snapshot',snapshot:{source_cards:[{source_id:'local.project-fin',source_kind:'project_agent',title:'builder',state:'completed',summary:'delegated task completed',current_activity:'delegated task completed',recent_actions:[{tool_call_id:'tool-done',tool_name:'project.tool',category:'tools',summary:'delegated task completed',status:'completed'}]}]}})
cards=renderAgentCards()
assert(cards[0].kind==='idle','completed project agent button should cool down to idle tone')
pins=renderPinnedAgentCards()
assert(pins[0].summary==='delegated task completed','completed pinned card must preserve completion summary')
assert(pins[0].actions[0].status==='completed','completed pinned card must preserve completed action status')
assert(nativeSent.some(m=>m.type==='session.bind'&&m.session_id==='session-1'),'native ws send path missing')
assert(nativeSent.some(m=>m.type==='session.command'&&m.command==='/new'),'new session command missing')
assert(nativeSent.some(m=>m.type==='session.rename'&&m.session_id==='s1'&&m.title==='新任务名'),'session rename message missing')
assert(nativeSent.some(m=>m.type==='session.archive'&&m.session_ids.includes('s1')),'session archive message missing')
assert(nativeSent.some(m=>m.type==='session.delete'&&m.session_ids.includes('s1')&&m.session_ids.includes('s2')),'session multi-delete message missing')
onWs({type:'unknown.event'})
assert(S.conn!=='protocol_mismatch','unknown event must not masquerade as protocol mismatch')
onWs({type:'protocol.error',reason:'unknown_message_type',message_type:'session.delete'})
assert(S.conn!=='protocol_mismatch','protocol.error must not masquerade as handshake mismatch')
console.log('SMOKE_OK')
