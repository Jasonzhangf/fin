#!/usr/bin/env python3
import asyncio, json, datetime, pathlib, time
import websockets
from websockets.exceptions import ConnectionClosed

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT/'reports/android-mvp-logs'
SHOT = ROOT/'reports/android-mvp-screenshots'
LOG.mkdir(parents=True, exist_ok=True)
SHOT.mkdir(parents=True, exist_ok=True)

WS='ws://100.66.1.82:4040/ws'

def check(status, name, ok, detail=None):
    item={'name':name,'ok':bool(ok)}
    if detail is not None:
        item['detail']=detail
    status['checks'].append(item)

async def recv_turn(ws, status, timeout_sec=120):
    deadline=time.time()+timeout_sec
    while time.time()<deadline:
        try:
            msg=json.loads(await asyncio.wait_for(ws.recv(), timeout=20))
            status['received'].append(msg.get('type'))
            if msg.get('type')=='turn.rendered':
                return msg
        except TimeoutError:
            continue
    raise TimeoutError('turn.rendered timeout')

async def main():
    status={
        'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),
        'ok': False,
        'ws': WS,
        'received': [],
        'checks': [],
        'turn_samples': [],
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
            await ws.send(json.dumps({'type':'session.user_input','session_id':session_id,'payload':p,'client_message_id':f'm-{int(time.time()*1000)}','timestamp_bucket':int(time.time())}))
            turns.append(await recv_turn(ws,status))

        # E2/E3: try to force tool+error records with bounded attempts
        advanced_prompts=[
            '请调用session.list工具并返回结果（若失败请输出失败原因）',
            '请用shell执行命令：nonexistent_command_abc123，并返回完整错误',
        ]
        for p in advanced_prompts:
            await ws.send(json.dumps({'type':'session.user_input','session_id':session_id,'payload':p,'client_message_id':f'm-{int(time.time()*1000)}','timestamp_bucket':int(time.time())}))
            turns.append(await recv_turn(ws,status))

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
        check(status,'E2_has_non_empty_tool_records',any(len(t.get('tool_execution_records') or [])>0 for t in turns))
        check(status,'E3_has_non_empty_error_records',any(len(t.get('error_records') or [])>0 for t in turns))
        check(status,'E4_multi_turn_stability',len(turns)>=4)
        check(status,'tool_records_type',all(isinstance(t.get('tool_execution_records',[]), list) for t in turns))
        check(status,'error_records_type',all(isinstance(t.get('error_records',[]), list) for t in turns))

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
