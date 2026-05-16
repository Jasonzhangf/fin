#!/usr/bin/env python3
import subprocess, time, pathlib, json, re

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG_DIR = ROOT / 'reports/android-mvp-logs'
SHOT_DIR = ROOT / 'reports/android-mvp-screenshots'
LOG_DIR.mkdir(parents=True, exist_ok=True)
SHOT_DIR.mkdir(parents=True, exist_ok=True)
ADB = '100.127.23.27:1234'
PKG = 'com.fin.client'


def sh(cmd):
    return subprocess.run(cmd, shell=True, text=True, capture_output=True)


def must(cmd, out=None):
    r = sh(cmd)
    if out:
        pathlib.Path(out).write_text((r.stdout or '') + (r.stderr or ''))
    if r.returncode != 0:
        raise RuntimeError(f"cmd failed: {cmd}\n{r.stdout}\n{r.stderr}")
    return r.stdout

def must_bin(cmd, out):
    with open(out, 'wb') as f:
        p = subprocess.run(cmd, shell=True, stdout=f, stderr=subprocess.PIPE)
    if p.returncode != 0:
        raise RuntimeError(f"cmd failed: {cmd}\n{p.stderr.decode(errors='ignore')}")


def ensure_daemon():
    def host_tcp_ok() -> bool:
        probe = sh(
            "python3 - <<'PP'\n"
            "import socket\n"
            "s=socket.socket(); s.settimeout(1)\n"
            "try:\n"
            " s.connect(('100.66.1.82',4040)); print('ok')\n"
            "except Exception:\n"
            " print('fail')\n"
            "PP"
        )
        return "ok" in (probe.stdout or "")

    pid_file = LOG_DIR / 'web-debug.pid'
    if pid_file.exists():
        pid = pid_file.read_text().strip()
        if pid and sh(f'ps -p {pid}').returncode == 0 and host_tcp_ok():
            return
    r = sh(f"nohup {ROOT}/rust/target/debug/fin-cli web-debug ~/.fin/config/user.toml 0.0.0.0 4040 > {LOG_DIR}/web-debug-live.log 2>&1 & echo $!")
    pid = (r.stdout or '').strip().splitlines()[-1]
    pid_file.write_text(pid + '\n')
    time.sleep(1.5)


def install_start_app():
    must(f'adb -s {ADB} install -r {ROOT}/android-client/update-dist/fin-latest-debug.apk', out=LOG_DIR/'adb-install.log')
    sh(f'adb -s {ADB} shell input keyevent KEYCODE_WAKEUP')
    sh(f'adb -s {ADB} shell wm dismiss-keyguard')
    sh(f'adb -s {ADB} shell svc power stayon true')
    sh(f'adb -s {ADB} shell am force-stop {PKG}')
    must(f'adb -s {ADB} shell am start -W -n {PKG}/.MainActivity', out=LOG_DIR/'adb-start-foreground.log')


def collect():
    time.sleep(10)
    must(f'adb -s {ADB} shell run-as {PKG} cat files/logs/connection-events.log', out=LOG_DIR/'tailscale-connection-events.log')
    must(f'adb -s {ADB} shell run-as {PKG} cat files/config/ws_profiles.json', out=LOG_DIR/'tailscale-config.json')
    must_bin(f'adb -s {ADB} exec-out screencap -p', SHOT_DIR/'tailscale-live-e2e.png')


def judge():
    text = (LOG_DIR/'tailscale-connection-events.log').read_text()
    ok = all(k in text for k in ['handshake=ok', 'state=subscribed', 'state=healthy'])
    status = {
        'ok': ok,
        'required': ['handshake=ok', 'state=subscribed', 'state=healthy'],
        'log_path': str(LOG_DIR/'tailscale-connection-events.log'),
        'screenshot': str(SHOT_DIR/'tailscale-live-e2e.png'),
    }
    (LOG_DIR/'tailscale-live-e2e-status.json').write_text(json.dumps(status, ensure_ascii=False, indent=2))
    return ok


def main():
    ensure_daemon()
    install_start_app()
    collect()
    ok = judge()
    print('PASS' if ok else 'FAIL')
    return 0 if ok else 1


if __name__ == '__main__':
    raise SystemExit(main())
