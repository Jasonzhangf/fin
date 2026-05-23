#!/usr/bin/env node
const S={conn:'idle',turns:[],pendingById:{},traceByClientId:{},itemByClientId:{},turnStateByClientId:{},runtimeHealth:'schema_error:runtime.health',providerHealth:'schema_error:provider.health',cache:''};
function schema(v,field){const s=(v&&v[field]!=null)?String(v[field]).trim():'';return s?s:'schema_error:'+field}
function setConn(s){S.conn=s}
function setRuntimeState(s){S.runtimeHealth=s}
function setProviderState(s){S.providerHealth=s}
function persistProviderConfigCache(v){S.cache=JSON.stringify(v||{})}
function itemFromRecord(rec){return {item_id:schema(rec,'tool_call_id'),label:schema(rec,'tool_name'),title:schema(rec,'title'),purpose:schema(rec,'purpose'),status:schema(rec,'status'),duration_ms:rec?.duration_ms??0,output_summary:rec?.output_summary||'',error_summary:rec?.error_summary||'',item_kind:rec?.tool_kind||'schema_error:item_kind'}}
function normalizeToolRecord(rec){return itemFromRecord(rec||{})}
function normalizeErrorRecord(e){const item=itemFromRecord(e||{});item.status='failed';item.error_summary=schema(e||{},'error_summary');return item}
function toTurn(t){return {u:t.user_input||'',a:t.assistant_response||'',control:t.control_feedback_summary||'',tool:t.tool_execution_summary||'',closure:t.closure_stop_source||'',items:(t.tool_execution_records||[]).map(normalizeToolRecord).concat((t.error_records||[]).map(normalizeErrorRecord))}}
function upsertItem(id,m){if(!id)return;const by=S.itemByClientId[id]||(S.itemByClientId[id]={});const itemId=schema(m,'item_id');const prev=by[itemId]||{};by[itemId]=Object.assign({},prev,m,{item_id:itemId,label:schema(m,'label'),title:schema(m,'title'),purpose:schema(m,'purpose'),status:schema(m,'status')});}
function consumeItems(id){const by=S.itemByClientId[id]||{};delete S.itemByClientId[id];return Object.values(by)}
function onWs(m){
  switch(m.type){
    case 'config.snapshot': persistProviderConfigCache(m); return;
    case 'input.accepted': {const id=m.client_message_id||''; if(id){S.pendingById[id]={state:'accepted',serverPhase:'queued'}} return;}
    case 'runtime.health': setRuntimeState(schema(m,'status')); return;
    case 'provider.health': setProviderState(schema(m,'status')); return;
    case 'turn.started': {const id=m.client_message_id||''; if(id)S.turnStateByClientId[id]=m; setRuntimeState('running'); return;}
    case 'turn.progress': {const id=m.client_message_id||''; if(id&&S.pendingById[id]){S.pendingById[id].state='waiting';S.pendingById[id].serverPhase=m.phase||'inference_waiting'} return;}
    case 'turn.item.started':
    case 'turn.item.delta':
    case 'turn.item.completed':
    case 'turn.item.failed': {const id=m.client_message_id||''; upsertItem(id,m); return;}
    case 'turn.completed': {setRuntimeState(m.status==='failed'?'failed':'ok'); return;}
    case 'turn.tool_event': {const id=m.client_message_id||''; if(id&&m.item)upsertItem(id,m.item); return;}
    case 'turn.trace_event': {const id=m.client_message_id||''; if(id){(S.traceByClientId[id]??=[]).push(m)} return;}
    case 'turn.error_event': {const id=m.client_message_id||''; if(id&&m.item)upsertItem(id,m.item); setRuntimeState('failed'); return;}
    case 'turn.rendered': {const id=m.client_message_id||''; if(id&&S.pendingById[id]) delete S.pendingById[id]; const t=toTurn(m); if(id){t.items=t.items.concat(consumeItems(id)); delete S.turnStateByClientId[id];} if(id&&S.traceByClientId[id]){t.control=(t.control||'')+';trace_events='+String(S.traceByClientId[id].length); delete S.traceByClientId[id];} S.turns.push(t); return;}
    default: setConn('protocol_unknown_event'); return;
  }
}
function assert(cond,msg){if(!cond) throw new Error(msg)}
function assertItem(item){
  for(const f of ['item_id','label','title','purpose','status']) assert(item[f] && !String(item[f]).startsWith('schema_error:'), `bad ${f}: ${JSON.stringify(item)}`)
  assert(!['tool','unknown',''].includes(String(item.label).trim()), 'non-informative label')
  assert(!['tool','unknown',''].includes(String(item.title).trim()), 'non-informative title')
}
onWs({type:'config.snapshot',default_profile:'p1',profiles:[{profile_name:'p1',provider:'mimo',model:'mimo',active:true}],active_thinking_effort:'high'})
onWs({type:'input.accepted',client_message_id:'m1'})
onWs({type:'runtime.health',status:'available'})
onWs({type:'provider.health',status:'available',provider:'mimo'})
onWs({type:'turn.started',client_message_id:'m1',session_id:'s1',turn_id:'t1'})
onWs({type:'turn.progress',client_message_id:'m1',phase:'tool_wait'})
onWs({type:'turn.item.started',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i1',item_kind:'provider',label:'provider.call',title:'Provider Call',purpose:'dispatch compiled prompt',status:'running',started_at:'now'})
onWs({type:'turn.item.completed',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i1',item_kind:'provider',label:'provider.call',title:'Provider Call',purpose:'dispatch compiled prompt',status:'completed',duration_ms:12,output_summary:'ok',error_summary:null})
onWs({type:'turn.item.started',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i2',item_kind:'exec',label:'shell.exec',title:'Execute shell',purpose:'run local command',status:'running',started_at:'now'})
onWs({type:'turn.item.failed',client_message_id:'m1',session_id:'s1',turn_id:'t1',item_id:'i2',item_kind:'exec',label:'shell.exec',title:'Execute shell',purpose:'run local command',status:'failed',duration_ms:3,error_summary:'command not found'})
onWs({type:'turn.trace_event',client_message_id:'m1',detail:'trace'})
onWs({type:'turn.completed',client_message_id:'m1',session_id:'s1',turn_id:'t1',status:'completed'})
onWs({type:'turn.rendered',client_message_id:'m1',user_input:'hi',assistant_response:'ok'})
assert(S.turns.length===1,'turn count mismatch')
assert(S.turns[0].items.length===2,'item timeline merge mismatch')
S.turns[0].items.forEach(assertItem)
assert(S.turns[0].items.some(i=>i.status==='failed' && i.error_summary==='command not found'),'failed item error missing')
assert(S.turns[0].control.includes('trace_events=1'),'trace merge mismatch')
assert(S.cache.includes('default_profile'),'config snapshot not persisted')
assert(S.runtimeHealth==='ok','runtime health not separated')
assert(S.providerHealth==='available','provider health not separated')
onWs({type:'unknown.event'})
assert(S.conn==='protocol_unknown_event','unknown event not explicit fail')
console.log('SMOKE_OK')
