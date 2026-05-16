#!/usr/bin/env python3
import json, pathlib, subprocess, datetime
import re

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT / 'reports/android-mvp-logs'
LOG.mkdir(parents=True, exist_ok=True)
ADB='100.127.23.27:1234'
PKG='com.fin.client'
TARGET_HOST='100.66.1.82'
TARGET_PORT=4040


def run(cmd):
    p = subprocess.run(cmd, shell=True, text=True, capture_output=True)
    return p.returncode, (p.stdout or '') + (p.stderr or '')


def main():
    checks=[]
    # app uid
    c,o = run(f"adb -s {ADB} shell cmd package list packages -U | grep {PKG}")
    m = re.search(r"uid:(\d+)", o or "")
    if not m:
        c2,o2 = run(f"adb -s {ADB} shell dumpsys package {PKG}")
        m = re.search(r"userId=(\d+)", o2 or "")
        o = (o or "") + ("\n" + o2 if o2 else "")
    app_uid = m.group(1) if m else ""
    checks.append({'name':'app_uid_detected', 'ok': bool(app_uid), 'raw': (o or '').strip()})
    # daemon listen check on host
    c,o = run(f"python3 - <<'PP'\nimport socket\ns=socket.socket(); s.settimeout(1)\ntry:\n s.connect(('{TARGET_HOST}',{TARGET_PORT})); print('host_tcp=ok')\nexcept Exception as e:\n print('host_tcp=fail',e)\nPP")
    checks.append({'name':'host_tcp', 'ok':'host_tcp=ok' in o, 'raw':o.strip()})

    # device shell tcp
    c,o = run(f"adb -s {ADB} shell 'nc -z -w 2 {TARGET_HOST} {TARGET_PORT}; echo exit=$?' ")
    checks.append({'name':'device_shell_tcp', 'ok':'exit=0' in o, 'raw':o.strip()})

    # app uid tcp (same uid path as app process, avoid only-shell false positive)
    c,o = run(f"adb -s {ADB} shell run-as {PKG} sh -c 'echo | nc -w 2 {TARGET_HOST} {TARGET_PORT} >/dev/null 2>&1; echo exit=$?'")
    checks.append({'name':'app_uid_shell_tcp', 'ok':'exit=0' in o, 'raw':o.strip()})

    # app-side latest probe snapshot
    c,o = run(f"adb -s {ADB} shell run-as {PKG} cat files/logs/connection-events.log")
    tail='\n'.join((o.splitlines()[-30:] if o else []))
    checks.append({'name':'app_probe_tail_has_healthy', 'ok': all(k in o for k in ['handshake=ok','state=subscribed','state=healthy']), 'raw':tail})

    # vpn uid include check (tailscale per-app path)
    c,o = run(f"adb -s {ADB} shell dumpsys connectivity | grep -m1 'VPN CONNECTED extra: VPN:com.tailscale.ipn' -A 4")
    vpn_dump = (o or "").strip()
    vpn_uid_ok = False
    if app_uid and vpn_dump:
        vpn_uid_ok = app_uid in vpn_dump or "Uids: <{" in vpn_dump
    checks.append({'name':'tailscale_vpn_uid_included', 'ok': vpn_uid_ok, 'raw': vpn_dump})

    # classify blocker
    host_ok=checks[1]['ok']; shell_ok=checks[2]['ok']; app_uid_ok=checks[3]['ok']; app_ok=checks[4]['ok']; vpn_uid_ok=checks[5]['ok']
    if not host_ok:
        blocker='daemon_not_listening_or_host_route_issue'
    elif host_ok and not shell_ok:
        blocker='device_to_tailscale_route_issue'
    elif host_ok and shell_ok and not vpn_uid_ok:
        blocker='tailscale_per_app_vpn_exclusion'
    elif host_ok and shell_ok and not app_uid_ok:
        blocker='app_uid_path_blocked'
    elif host_ok and shell_ok and app_uid_ok and not app_ok:
        blocker='webview_or_app_runtime_ws_path_issue'
    else:
        blocker='none'

    payload={
        'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),
        'target': f'{TARGET_HOST}:{TARGET_PORT}',
        'blocker': blocker,
        'app_uid': app_uid,
        'checks': checks,
    }
    out=LOG/'preflight-network-diagnose.json'
    out.write_text(json.dumps(payload, ensure_ascii=False, indent=2))
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0

if __name__=='__main__':
    raise SystemExit(main())
