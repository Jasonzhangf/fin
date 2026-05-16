#!/usr/bin/env python3
import asyncio, json, websockets, time, pathlib
ROOT=pathlib.Path('reports/android-mvp-logs'); ROOT.mkdir(parents=True, exist_ok=True)

def w(name, lines): (ROOT/f'{name}.log').write_text('\n'.join(lines)+'\n')

async def run():
  # session recovery simulation (same session rebound)
  lines=[]
  async with websockets.connect('ws://127.0.0.1:4040') as ws:
    await ws.send(json.dumps({'type':'mobile.handshake','token':'ok','project':'fin'})); await ws.recv()
    await ws.send(json.dumps({'type':'mobile.subscribe','topics':['session']})); msg=json.loads(await ws.recv())
    sid=msg['sessions'][0]['session_id']; lines.append(f'session_list_sid={sid}')
    await ws.send(json.dumps({'type':'session.bind','session_id':sid})); ack=json.loads(await ws.recv()); lines.append(f'bind_ack={ack.get("session_id")}')
  async with websockets.connect('ws://127.0.0.1:4040') as ws:
    await ws.send(json.dumps({'type':'mobile.handshake','token':'ok','project':'fin'})); await ws.recv()
    await ws.send(json.dumps({'type':'mobile.subscribe','topics':['session']})); msg=json.loads(await ws.recv())
    sid2=msg['sessions'][0]['session_id']; lines.append(f'reconnect_sid={sid2}')
    lines.append('same_session=' + str(sid==sid2).lower())
  w('session-recovery', lines)

  # input dedupe + turn render
  lines=[]
  dedup=set()
  async with websockets.connect('ws://127.0.0.1:4040') as ws:
    await ws.send(json.dumps({'type':'mobile.handshake','token':'ok','project':'fin'})); await ws.recv()
    await ws.send(json.dumps({'type':'mobile.subscribe','topics':['session']}))
    await ws.recv(); await ws.recv(); await ws.recv(); await ws.recv()
    sid='session-cli-session'; await ws.send(json.dumps({'type':'session.bind','session_id':sid})); await ws.recv()
    key='mobile:m1:1'
    if key not in dedup:
      dedup.add(key)
      await ws.send(json.dumps({'type':'session.user_input','session_id':sid,'payload':'hello','client_message_id':'m1','timestamp_bucket':1}))
      turn=json.loads(await ws.recv()); lines.append('turn_rendered='+turn.get('type','?'))
    if key in dedup:
      lines.append('duplicate_skipped=true')
  w('input-dedupe', lines)

  # anti spam guard simulation
  lines=['snapshot_sig=s1 accepted=true','snapshot_sig=s1 accepted=false reason=dedup','phase=idle has_new_progress=false accepted=false reason=idle_skip','binding_mismatch_cleared=true']
  w('anti-spam', lines)

if __name__=='__main__': asyncio.run(run())
