#!/usr/bin/env python3
import json, os, socket, subprocess, time, urllib.request, urllib.error
from pathlib import Path
ROOT=Path('/Volumes/extension/code/fin')
OUT=ROOT/'reports/regression/android-log-ingest-e2e'; OUT.mkdir(parents=True, exist_ok=True)

def free_port():
 s=socket.socket(); s.bind(('127.0.0.1',0)); p=s.getsockname()[1]; s.close(); return p

def wait_http(url, timeout=15.0):
 d=time.time()+timeout
 while time.time()<d:
  try:
   with urllib.request.urlopen(url, timeout=1.5) as r:
    if r.status in (200,404): return True
  except Exception: time.sleep(0.3)
 return False

def finalize(res, code):
 (OUT/'gate.json').write_text(json.dumps(res,ensure_ascii=False,indent=2),encoding='utf-8')
 print(json.dumps({'status':res['status'],'errors':res['errors']},ensure_ascii=False))
 return code

def main():
 runtime_home=Path(os.environ.get('FIN_RUNTIME_HOME_OVERRIDE', str(Path.home()/'.fin')))
 bind=f'127.0.0.1:{free_port()}'
 req={'ts':time.time(),'source':'android-test','event':'diag.click.e2e'}
 res={'bind':bind,'runtime_home':str(runtime_home),'request':req,'status':'FAIL','errors':[]}
 proc=subprocess.Popen(['cargo','run','-p','fin-cli','--manifest-path','rust/Cargo.toml','--','web-debug','--bind',bind],cwd=str(ROOT),stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 try:
  if not wait_http(f'http://{bind}/api/binding.json',20):
   res['errors'].append('daemon_debug_server_not_ready'); return finalize(res,1)
  body=json.dumps(req).encode('utf-8')
  post=urllib.request.Request(f'http://{bind}/api/log/ingest',data=body,headers={'Content-Type':'application/json; charset=utf-8'},method='POST')
  try:
   with urllib.request.urlopen(post, timeout=5) as r:
    res['post_status']=r.status; res['post_body']=r.read().decode('utf-8','replace')
  except urllib.error.HTTPError as e:
   res['post_status']=e.code; res['post_body']=e.read().decode('utf-8','replace'); res['errors'].append(f'post_http_{e.code}'); return finalize(res,1)
  with urllib.request.urlopen(f'http://{bind}/api/log/latest', timeout=5) as r:
   raw=r.read().decode('utf-8','replace'); res['get_status']=r.status; res['get_body']=raw; payload=json.loads(raw)
  ev=payload.get('events') if isinstance(payload,dict) else None
  if not isinstance(ev,list): res['errors'].append('invalid_events_payload'); return finalize(res,1)
  if not any('diag.click.e2e' in line for line in ev): res['errors'].append('event_not_found_in_latest'); return finalize(res,1)
  logf=runtime_home/'runtime/logs/mobile/connection-events.logl'; res['log_file']=str(logf); res['log_file_exists']=logf.exists()
  if not logf.exists(): res['errors'].append('log_file_not_created'); return finalize(res,1)
  res['status']='PASS'; return finalize(res,0)
 finally:
  try: proc.terminate(); proc.wait(timeout=5)
  except Exception: pass

if __name__=='__main__': raise SystemExit(main())
