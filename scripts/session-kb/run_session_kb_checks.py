#!/usr/bin/env python3
import asyncio, json, pathlib, datetime, time, re, os
import websockets

ROOT=pathlib.Path('/Volumes/extension/code/fin')
LOG=ROOT/'reports/session-kb-logs'
LOG.mkdir(parents=True,exist_ok=True)
WS=os.environ.get('WS_URL','ws://100.66.1.82:4040/ws')

async def run():
    st={'ts':datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),'checks':[],'events':[]}
    def add(name,ok,detail=''):
        st['checks'].append({'name':name,'ok':bool(ok),'detail':detail})

    async with websockets.connect(WS,max_size=8_000_000,ping_interval=None,open_timeout=20) as ws:
        await ws.send(json.dumps({'type':'mobile.handshake','token':'','project':'fin','scopes':['session.read','session.write','runtime.read']}))
        await ws.send(json.dumps({'type':'mobile.subscribe','topics':['session','runtime','project','daemon']}))

        async def recv_until(want, timeout=20):
            end=time.time()+timeout
            while time.time()<end:
                try:
                    m=json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
                except TimeoutError:
                    continue
                t=m.get('type','')
                st['events'].append(t)
                if t==want:
                    return m
            return None

        async def wait_session_row(session_id, timeout=20):
            end=time.time()+timeout
            last_row=None
            while time.time()<end:
                m = await recv_until('session.list', timeout=max(1, int(end-time.time())))
                if not m:
                    break
                for row in m.get('sessions',[]) or []:
                    if isinstance(row, dict) and row.get('session_id') == session_id:
                        last_row=row
                        return row
            return last_row

        lst=await recv_until('session.list', timeout=25)
        add('C1_session_list_present', lst is not None)
        sessions=(lst or {}).get('sessions',[])
        add('C1_session_list_non_empty', len(sessions)>0, f'count={len(sessions)}')
        if sessions:
            keys=['session_id','task_id','updated_at','title','preview_100']
            add('C1_session_list_fields', all(k in sessions[0] for k in keys), str(list(sessions[0].keys())))

        # U1 default binding behavior via explicit bind fallback
        fallback='session-system-entry'
        target=sessions[0]['session_id'] if sessions else fallback
        if sessions and any(s.get('session_id')==fallback for s in sessions):
            target=fallback
        await ws.send(json.dumps({'type':'session.bind','session_id':target}))
        b=await recv_until('session.bound', timeout=10)
        h=await recv_until('session.history', timeout=15)
        add('U1_bind_success', b is not None, str((b or {}).get('session_id')))
        add('U2_history_matches_bound', h is not None and (h.get('session_id')==target), f"history={(h or {}).get('session_id')} target={target}")

        # U3 CRUD
        await ws.send(json.dumps({'type':'session.command','command':'/new'}))
        r=await recv_until('session.command.result', timeout=20)
        add('U3_new_ok', r is not None and 'new session ready:' in (r.get('answer','') if r else ''), (r or {}).get('answer',''))
        new_from_reply = None
        if r:
            m = re.search(r'(session-\d{14,})', r.get('answer',''))
            if m:
                new_from_reply = m.group(1)
        lst2=await recv_until('session.list', timeout=20)
        sessions2=(lst2 or {}).get('sessions',[])
        new_sid=new_from_reply
        if not new_sid and sessions2:
            ids=[x.get('session_id') for x in sessions2 if isinstance(x,dict)]
            new_sid=next((i for i in ids if i and i.startswith('session-20')), None)
        add('U3_new_session_discovered', new_sid is not None, str(new_sid))

        if new_sid:
            await ws.send(json.dumps({'type':'session.command','command':f'/session rename {new_sid} demo-title'}))
            rr=await recv_until('session.command.result', timeout=20)
            add('U3_rename_ok', rr is not None and 'session renamed:' in (rr.get('answer','') if rr else ''), (rr or {}).get('answer',''))
            await ws.send(json.dumps({'type':'session.command','command':'/sessions'}))
            _ = await recv_until('session.command.result', timeout=20)
            row=await wait_session_row(new_sid, timeout=20)
            add('C1_title_updated', row is not None and row.get('title')=='demo-title', str((row or {}).get('title')))

            await ws.send(json.dumps({'type':'session.command','command':f'/session archive {new_sid}'}))
            ar=await recv_until('session.command.result', timeout=20)
            add('U3_archive_ok', ar is not None and 'session archived:' in (ar.get('answer','') if ar else ''), (ar or {}).get('answer',''))

            await ws.send(json.dumps({'type':'session.command','command':f'/session delete {new_sid}'}))
            dr=await recv_until('session.command.result', timeout=25)
            add('U4_delete_with_extract_ok', dr is not None and 'deleted' in (dr.get('answer','') if dr else ''), (dr or {}).get('answer',''))
            lst4=await recv_until('session.list', timeout=20)
            exists=any(x.get('session_id')==new_sid for x in (lst4 or {}).get('sessions',[]))
            add('U4_deleted_not_in_list', not exists, f'exists={exists}')

        # C2 knowledge index
        kb_idx=pathlib.Path('/Users/fanzhang/.fin/projects/fin/knowledge-base/indexes/by_session.json')
        add('C2_kb_index_exists', kb_idx.exists(), str(kb_idx))
        if kb_idx.exists():
            try:
                data=json.loads(kb_idx.read_text())
                add('C2_kb_index_json_object', isinstance(data,dict), str(type(data)))
            except Exception as e:
                add('C2_kb_index_json_object', False, str(e))

    st['ok']=all(c['ok'] for c in st['checks'])
    (LOG/'session-kb-checks.log').write_text(json.dumps(st,ensure_ascii=False,indent=2))
    print(json.dumps(st,ensure_ascii=False,indent=2))
    return 0 if st['ok'] else 1

if __name__=='__main__':
    raise SystemExit(asyncio.run(run()))
