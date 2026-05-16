#!/usr/bin/env python3
import asyncio, json, websockets

async def handler(ws):
    async for msg in ws:
        data=json.loads(msg)
        t=data.get('type')
        if t=='mobile.handshake':
            token=data.get('token','')
            if token=='bad':
                await ws.send(json.dumps({'type':'handshake.auth_failed'})); continue
            if data.get('project')=='bad-protocol':
                await ws.send(json.dumps({'type':'handshake.protocol_mismatch'})); continue
            await ws.send(json.dumps({'type':'handshake.ok'}))
        elif t=='mobile.subscribe':
            await ws.send(json.dumps({'type':'session.list','sessions':[{'session_id':'session-cli-session','task_id':'task-1','topic':'MVP','phase':'ready','updated_at':'2026-05-16T07:00:00+08:00','project':'fin'}]}))
            await ws.send(json.dumps({'type':'runtime.workers','workers':[{'worker_id':'w1','presence':'idle','heartbeat_at':'2026-05-16T07:00:01+08:00','current_task_id':None}]}))
            await ws.send(json.dumps({'type':'runtime.projects','projects':[{'project_id':'fin','supervision_action':'resume','wake_queue_state':'empty','pickup_state':'ready_to_resume'}]}))
            await ws.send(json.dumps({'type':'runtime.daemon','daemon':{'lifecycle_state':'running','stale_lease':False}}))
        elif t=='session.bind':
            await ws.send(json.dumps({'type':'session.bound','session_id':data.get('session_id')}))
        elif t=='session.user_input':
            await ws.send(json.dumps({'type':'turn.rendered','turn_id':'t1','user_input':data.get('payload',''),'assistant_response':'ok','control_feedback_summary':'continue','tool_execution_summary':'none','closure_stop_source':'end_turn'}))

async def main():
    async with websockets.serve(handler,'127.0.0.1',4040):
        print('mock ws server listening on ws://127.0.0.1:4040')
        await asyncio.Future()

if __name__=='__main__':
    asyncio.run(main())
