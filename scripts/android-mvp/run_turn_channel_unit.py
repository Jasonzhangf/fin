#!/usr/bin/env python3
import json, pathlib, datetime, re

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT / 'reports/android-mvp-logs'
LOG.mkdir(parents=True, exist_ok=True)
html = (ROOT/'android-client/app/src/main/assets/mobile-shell.html').read_text()

checks=[]

def add(name, ok, detail=''):
    checks.append({'name':name,'ok':bool(ok),'detail':detail})

# U1: same turn has normal base render and debug conditional render branch
add('U1_normal_base_render', '你：${esc(t.u)}' in html and "<div class='assistant'>${esc(t.a)}</div>" in html)
add('U1_debug_conditional_branch', 'S.debugMode&&' in html and '调试信息' in html and 'toolRecords' in html and 'errors' in html)

# U2: toggle debug does not mutate turn store
m = re.search(r'function toggleDebugMode\(\)\{([\s\S]*?)\n\}', html)
body = m.group(1) if m else ''
add('U2_toggle_no_turn_store_mutation', ('S.turns.push' not in body and 'S.turns=' not in body and 'splice(' not in body and 'pop(' not in body), body.strip()[:180])

# U3: send payload does not depend on debugMode
m2 = re.search(r'function sendInput\(\)\{([\s\S]*?)\n\}', html)
send_body = m2.group(1) if m2 else ''
add('U3_send_payload_independent_from_debug', 'debugMode' not in send_body, send_body.strip()[:180])

status = {
    'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),
    'ok': all(c['ok'] for c in checks),
    'checks': checks,
}
(LOG/'turn-channel-unit.log').write_text(json.dumps(status, ensure_ascii=False, indent=2))
print(json.dumps(status, ensure_ascii=False, indent=2))
raise SystemExit(0 if status['ok'] else 1)
