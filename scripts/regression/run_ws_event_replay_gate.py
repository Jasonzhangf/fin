#!/usr/bin/env python3
import asyncio, json, datetime
import websockets

HOST='127.0.0.1'
PORT=18440

TURN_PAYLOAD={
  'type':'turn.rendered',
  'turn_id':'t-replay-1',
  'user_input':'replay test',
  'assistant_response':'ok',
  'control_feedback_summary':'continue',
  'tool_execution_summary':'4 events',
  'tool_execution_records':[
    {'tool_name':'exec_command','status':'start','event':'start'},
    {'tool_name':'exec_command','status':'progress','event':'progress'},
    {'tool_name':'exec_command','status':'completed','event':'completed'},
    {'tool_name':'exec_command','status':'error','event':'error'},
    {'status':'completed','event':'completed','raw':{'x':'missing_tool_name'}}
  ],
  'error_records':[{'message':'simulated tool error'}],
  'closure_stop_source':'end_turn',
}

async def handler(ws):
    async for raw in ws:
        msg=json.loads(raw)
        t=msg.get('type')
        if t=='mobile.handshake':
            await ws.send(json.dumps({'type':'handshake.ok'}))
        elif t=='mobile.subscribe':
            await ws.send(json.dumps({'type':'session.list','sessions':[{'session_id':'s1'}]}))
        elif t=='session.bind':
            await ws.send(json.dumps({'type':'session.bound','session_id':'s1'}))
        elif t=='session.user_input':
            await ws.send(json.dumps(TURN_PAYLOAD))

async def run_client(status):
    uri=f'ws://{HOST}:{PORT}'
    async with websockets.connect(uri, open_timeout=10, ping_interval=None) as ws:
        await ws.send(json.dumps({'type':'mobile.handshake','project':'fin'}))
        await ws.send(json.dumps({'type':'mobile.subscribe'}))
        # wait list then bind
        for _ in range(5):
            m=json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
            if m.get('type')=='session.list':
                await ws.send(json.dumps({'type':'session.bind','session_id':'s1'}))
                break
        await ws.send(json.dumps({'type':'session.user_input','session_id':'s1','payload':'go'}))
        turn=None
        for _ in range(10):
            m=json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
            if m.get('type')=='turn.rendered':
                turn=m; break
        assert turn is not None, 'missing turn.rendered'
        records=turn.get('tool_execution_records') or []
        statuses={r.get('status') for r in records if isinstance(r,dict)}
        status['checks']['tool_status_start']=('start' in statuses)
        status['checks']['tool_status_progress']=('progress' in statuses)
        status['checks']['tool_status_completed']=('completed' in statuses)
        status['checks']['tool_status_error']=('error' in statuses)
        status['checks']['missing_field_raw_present']=any(('tool_name' not in r and 'raw' in r) for r in records if isinstance(r,dict))
        status['checks']['error_records_present']=len(turn.get('error_records') or [])>0

async def main():
    status={'ts':datetime.datetime.utcnow().isoformat()+'Z','ok':False,'checks':{}}
    server=await websockets.serve(handler,HOST,PORT)
    try:
        await run_client(status)
    finally:
        server.close(); await server.wait_closed()
    status['ok']=all(status['checks'].values())
    print(json.dumps(status,ensure_ascii=False,indent=2))
    return 0 if status['ok'] else 1

if __name__=='__main__':
    raise SystemExit(asyncio.run(main()))
