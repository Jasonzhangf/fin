#!/usr/bin/env python3
import pathlib, subprocess, time, json, datetime
ROOT=pathlib.Path('/Volumes/extension/code/fin')
LOG=ROOT/'reports/android-mvp-logs'
LOG.mkdir(parents=True, exist_ok=True)
ADB='100.127.23.27:1234'
PKG='com.fin.client'

def sh(cmd):
    return subprocess.run(cmd,shell=True,text=True,capture_output=True)

def check_shell_tcp():
    r=sh(f"adb -s {ADB} shell 'nc -z -w 2 100.66.1.82 4040; echo exit=$?' ")
    txt=(r.stdout or '')+(r.stderr or '')
    return 'exit=0' in txt, txt.strip()

def check_app_markers():
    r=sh(f"adb -s {ADB} shell run-as {PKG} cat files/logs/connection-events.log")
    txt=(r.stdout or '')
    ok=all(k in txt for k in ['handshake=ok','state=subscribed','state=healthy'])
    return ok, txt.splitlines()[-20:]

def run_once():
    shell_ok,shell_raw=check_shell_tcp()
    app_ok,tail=check_app_markers()
    payload={
      'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),
      'shell_tcp_ok': shell_ok,
      'app_live_ok': app_ok,
      'shell_raw': shell_raw,
      'app_tail': tail,
    }
    (LOG/'live-unblock-observation.json').write_text(json.dumps(payload,ensure_ascii=False,indent=2))
    print(json.dumps(payload,ensure_ascii=False,indent=2))
    return 0 if app_ok else 1

if __name__=='__main__':
    raise SystemExit(run_once())
