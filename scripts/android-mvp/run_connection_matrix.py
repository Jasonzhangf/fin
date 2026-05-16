#!/usr/bin/env python3
import asyncio, json, websockets, time, pathlib
ROOT=pathlib.Path('reports/android-mvp-logs'); ROOT.mkdir(parents=True, exist_ok=True)

async def case(name, endpoint, token='ok', project='fin', exp=0):
    out=[]
    def log(x): out.append(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {x}")
    state='idle'; log('state=idle')
    state='scanned'; log('state=scanned')
    state='resolving'; log('state=resolving')
    if exp and int(time.time())>exp:
      state='stale_token'; log('state=stale_token');
      (ROOT/f'{name}.log').write_text('\n'.join(out)+'\n'); return
    state='connecting'; log('state=connecting')
    try:
      async with websockets.connect(endpoint) as ws:
        state='handshaking'; log('state=handshaking')
        await ws.send(json.dumps({'type':'mobile.handshake','token':token,'project':project}))
        msg=json.loads(await ws.recv())
        t=msg.get('type')
        if t=='handshake.auth_failed': state='auth_failed'; log('state=auth_failed')
        elif t=='handshake.protocol_mismatch': state='protocol_mismatch'; log('state=protocol_mismatch')
        elif t=='handshake.ok':
          await ws.send(json.dumps({'type':'mobile.subscribe','topics':['session','runtime','project','daemon']}))
          state='subscribed'; log('state=subscribed')
          # consume a few frames
          for _ in range(4):
            m=json.loads(await ws.recv()); log('event='+m.get('type','?'))
          state='healthy'; log('state=healthy')
    except Exception as e:
      state='endpoint_unreachable'; log('state=endpoint_unreachable err='+str(e))
    (ROOT/f'{name}.log').write_text('\n'.join(out)+'\n')

async def main():
  await case('connection-happy','ws://127.0.0.1:4040')
  await case('connection-auth-failed','ws://127.0.0.1:4040',token='bad')
  await case('connection-protocol-mismatch','ws://127.0.0.1:4040',project='bad-protocol')
  await case('connection-stale-token','ws://127.0.0.1:4040',exp=1)
  await case('connection-unreachable','ws://127.0.0.1:4999')

if __name__=='__main__': asyncio.run(main())
