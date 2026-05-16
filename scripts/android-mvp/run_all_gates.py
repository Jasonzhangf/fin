#!/usr/bin/env python3
import subprocess, json, pathlib, datetime

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT / 'reports/android-mvp-logs'
LOG.mkdir(parents=True, exist_ok=True)
OUT_JSON = LOG / 'all-gates-status.json'
OUT_MD = ROOT / 'reports/android-mvp-gate-summary.md'

CMDS = [
    ("preflight_network_diagnose", "python3 scripts/android-mvp/preflight_network_diagnose.py"),
    ("connection_matrix", "python3 scripts/android-mvp/run_connection_matrix.py"),
    ("session_input_matrix", "python3 scripts/android-mvp/run_session_input_matrix.py"),
    ("capture_shell_screenshots", "python3 scripts/android-mvp/capture_shell_screenshots.py"),
    ("assemble_debug", "cd android-client && ./gradlew :app:assembleDebug"),
    ("build_publish", "cd android-client && ./scripts/build-and-publish.sh"),
    ("tailscale_live_e2e", "python3 scripts/android-mvp/run_tailscale_live_e2e.py"),
]

def ensure_local_daemon():
    pid_file = LOG / 'web-debug.pid'
    if pid_file.exists():
        pid = pid_file.read_text().strip()
        if pid and subprocess.run(f'ps -p {pid}', shell=True, cwd=ROOT).returncode == 0:
            return
    p = subprocess.run(
        f"nohup {ROOT}/rust/target/debug/fin-cli web-debug ~/.fin/config/user.toml 127.0.0.1 4040 > {LOG}/web-debug-local.log 2>&1 & echo $!",
        shell=True,
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    (LOG / 'web-debug.pid').write_text((p.stdout or '').strip().splitlines()[-1] + '\n')

def run(name, cmd):
    p = subprocess.run(cmd, shell=True, cwd=ROOT, text=True, capture_output=True)
    (LOG / f'gate-{name}.log').write_text((p.stdout or '') + '\n' + (p.stderr or ''))
    return {
        'name': name,
        'cmd': cmd,
        'ok': p.returncode == 0,
        'code': p.returncode,
        'log': str(LOG / f'gate-{name}.log')
    }


def main():
    ensure_local_daemon()
    results = [run(n, c) for n, c in CMDS]
    preflight = next((r for r in results if r['name'] == 'preflight_network_diagnose'), None)
    preflight_blocker = None
    preflight_path = ROOT / 'reports/android-mvp-logs/preflight-network-diagnose.json'
    if preflight_path.exists():
        try:
            preflight_blocker = json.loads(preflight_path.read_text()).get('blocker')
        except Exception:
            preflight_blocker = 'unknown'
    if preflight is not None and preflight_blocker not in (None, "none"):
        preflight['ok'] = False
        if preflight['code'] == 0:
            preflight['code'] = 2
    all_ok = all(r['ok'] for r in results)
    payload = {
        'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00', 'Z'),
        'all_ok': all_ok,
        'preflight_blocker': preflight_blocker,
        'results': results,
    }
    OUT_JSON.write_text(json.dumps(payload, ensure_ascii=False, indent=2))

    lines = [
        '# Android MVP Gate Summary',
        '',
        f"- time: {payload['ts']}",
        f"- overall: {'PASS' if all_ok else 'FAIL'}",
        f"- preflight_blocker: {preflight_blocker or 'none'}",
        '',
        '| gate | status | log |',
        '|---|---|---|'
    ]
    for r in results:
        lines.append(f"| {r['name']} | {'✅' if r['ok'] else '❌'} | `{r['log']}` |")
    OUT_MD.write_text('\n'.join(lines) + '\n')
    # keep downstream reports in sync
    subprocess.run(
        "python3 scripts/android-mvp/update_validation_from_gates.py",
        shell=True,
        cwd=ROOT,
    )
    subprocess.run(
        "python3 scripts/android-mvp/objective_checklist.py",
        shell=True,
        cwd=ROOT,
    )
    print('PASS' if all_ok else 'FAIL')
    return 0 if all_ok else 1

if __name__ == '__main__':
    raise SystemExit(main())
