#!/usr/bin/env python3
import asyncio, json, datetime, pathlib, time, os
import websockets
from websockets.exceptions import ConnectionClosed

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT/'reports/android-mvp-logs'
SHOT = ROOT/'reports/android-mvp-screenshots'
LOG.mkdir(parents=True, exist_ok=True)
SHOT.mkdir(parents=True, exist_ok=True)

WS=os.environ.get('FIN_E2E_WS','ws://100.66.1.82:4040/ws')

def check(status, name, ok, detail=None):
    item={'name':name,'ok':bool(ok)}
    if detail is not None:
        item['detail']=detail
    status['checks'].append(item)

async def recv_turn(ws,status,timeout=120):
    deadline=time.time()+timeout
    while time.time()<deadline:
        try:
            msg=json.loads(await asyncio.wait_for(ws.recv(), timeout=10))
        except TimeoutError:
            continue
        status['received'].append(msg.get('type'))
        status['events'].append(msg)
        if msg.get('type')=='turn.rendered':
            return msg
    raise TimeoutError('turn.rendered timeout')

async def main():
    status={
        'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),
        'ok': False,
        'ws': WS,
        'received': [],
        'checks': [],
        'turn_samples': [],
        'events': [],
    }
    async with websockets.connect(WS, max_size=8_000_000, ping_interval=None, open_timeout=20) as ws:
        await ws.send(json.dumps({'type':'mobile.handshake','token':'','project':'fin','scopes':['session.read','session.write','runtime.read']}))
        await ws.send(json.dumps({'type':'mobile.subscribe','topics':['session','runtime','project','daemon']}))

        turns=[]
        session_id=None
        deadline=time.time()+40
        while time.time()<deadline:
            msg=json.loads(await asyncio.wait_for(ws.recv(), timeout=10))
            status['received'].append(msg.get('type'))
            if msg.get('type')=='session.list' and msg.get('sessions'):
                session_id=msg['sessions'][0].get('session_id')
                await ws.send(json.dumps({'type':'session.bind','session_id':session_id}))
                break
        check(status,'session_bind_ready',bool(session_id))
        if not session_id:
            (LOG/'turn-channel-e2e.log').write_text(json.dumps(status, ensure_ascii=False, indent=2))
            print(json.dumps(status, ensure_ascii=False, indent=2))
            return 1

        # E1: normal turns
        e1_prompts=['请直接回复OK','请总结你刚才回复的内容']
        for p in e1_prompts:
            client_id=f'm-{int(time.time()*1000)}'
            status['events'].append({'type':'client.send','client_message_id':client_id,'payload':p})
            await ws.send(json.dumps({'type':'session.user_input','session_id':session_id,'payload':p,'client_message_id':client_id,'timestamp_bucket':int(time.time())}))
            try:
                turns.append(await recv_turn(ws,status))
            except Exception:
                (LOG/'turn-channel-e2e.log').write_text(json.dumps(status, ensure_ascii=False, indent=2))
                raise

        # E2/E3: try to force tool+error records with bounded attempts
        advanced_prompts=[
            '请调用session.list工具并返回结果（若失败请输出失败原因）',
            '请用shell执行命令：nonexistent_command_abc123，并返回完整错误',
        ]
        for p in advanced_prompts:
            client_id=f'm-{int(time.time()*1000)}'
            status['events'].append({'type':'client.send','client_message_id':client_id,'payload':p})
            await ws.send(json.dumps({'type':'session.user_input','session_id':session_id,'payload':p,'client_message_id':client_id,'timestamp_bucket':int(time.time())}))
            try:
                turns.append(await recv_turn(ws,status))
            except Exception:
                (LOG/'turn-channel-e2e.log').write_text(json.dumps(status, ensure_ascii=False, indent=2))
                raise

        status['turn_count']=len(turns)
        required=['user_input','assistant_response','control_feedback_summary','tool_execution_summary','tool_execution_records','error_records','closure_stop_source']
        for idx,t in enumerate(turns,1):
            for k in required:
                check(status,f'turn{idx}_has_{k}',k in t)

        for t in turns:
            status['turn_samples'].append({
                'user_input': t.get('user_input','')[:80],
                'assistant_preview': t.get('assistant_response','')[:120],
                'tool_count': len(t.get('tool_execution_records') or []),
                'error_count': len(t.get('error_records') or []),
                'closure': t.get('closure_stop_source'),
            })

        # E1/E2/E3/E4 checks
        check(status,'E1_at_least_two_normal_turns',len(turns)>=2)
        check(status,'E2_has_non_empty_tool_records',any(len(t.get('tool_execution_records') or [])>0 for t in turns) or any(e.get('type')=='turn.item.completed' for e in status['events']))
        check(status,'E3_has_non_empty_error_records',any(len(t.get('error_records') or [])>0 for t in turns))
        check(status,'E4_multi_turn_stability',len(turns)>=4)
        check(status,'tool_records_type',all(isinstance(t.get('tool_execution_records',[]), list) for t in turns))
        check(status,'error_records_type',all(isinstance(t.get('error_records',[]), list) for t in turns))

        events=status['events']
        item_started=[e for e in events if e.get('type')=='turn.item.started']
        item_terminal=[e for e in events if e.get('type') in ('turn.item.completed','turn.item.failed')]
        terminal_by_id={e.get('item_id') for e in item_terminal if e.get('item_id')}
        check(status,'item_started_present',len(item_started)>0,{'count':len(item_started)})
        check(status,'item_terminal_present',len(item_terminal)>0,{'count':len(item_terminal)})
        check(status,'each_item_started_has_terminal',all(e.get('item_id') in terminal_by_id for e in item_started))
        for f in ('item_id','label','title','purpose','status'):
            check(status,f'item_started_has_{f}',all(str(e.get(f,'')).strip() for e in item_started))
        bad={'','tool','unknown'}
        check(status,'item_labels_informative',all(str(e.get('label','')).strip() not in bad for e in item_started))
        check(status,'item_titles_informative',all(str(e.get('title','')).strip() not in bad for e in item_started))
        failed_items=[e for e in item_terminal if e.get('type')=='turn.item.failed' or e.get('status')=='failed']
        check(status,'failed_items_keep_error_summary',all(str(e.get('error_summary') or '').strip() for e in failed_items),{'failed_count':len(failed_items)})
        check(status,'turn_started_present',any(e.get('type')=='turn.started' for e in events))
        check(status,'turn_completed_present',any(e.get('type')=='turn.completed' for e in events))

    status['ok']=all(c['ok'] for c in status['checks'])
    (LOG/'turn-channel-e2e.log').write_text(json.dumps(status, ensure_ascii=False, indent=2))
    print(json.dumps(status, ensure_ascii=False, indent=2))
    return 0 if status['ok'] else 1

if __name__=='__main__':
    async def with_retry():
        last_err=None
        for i in range(2):
            try:
                return await main()
            except ConnectionClosed as e:
                last_err=e
                time.sleep(1.2*(i+1))
                continue
        raise last_err if last_err else RuntimeError('unknown e2e failure')
    raise SystemExit(asyncio.run(with_retry()))
