#!/usr/bin/env python3
import json, pathlib, datetime

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT/'reports/android-mvp-logs'
LOG.mkdir(parents=True, exist_ok=True)
html = (ROOT/'android-client/app/src/main/assets/mobile-shell.html').read_text()

checks=[]

def add(name, ok):
    checks.append({'name':name,'ok':ok})

add('single_turn_store', 'const S={' in html and 'turns:[]' in html)
add('single_ingress_onWs_turn_rendered', 'if(m.type===\'turn.rendered\')' in html)
add('debug_toggle_exists', 'toggleDebugMode' in html and 'debugMode' in html)
add('toggle_no_connect_side_effect', ('connectWs(' not in html.split('function toggleDebugMode(){',1)[1].split('}',1)[0]))
add('toggle_no_send_side_effect', ('sendInput(' not in html.split('function toggleDebugMode(){',1)[1].split('}',1)[0]))
add('no_placeholder_thinking', '思考中' not in html)

ok = all(c['ok'] for c in checks)
status={
    'ts': datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),
    'ok': ok,
    'checks': checks,
}
(LOG/'turn-channel-toggle.log').write_text(json.dumps(status, ensure_ascii=False, indent=2))
print(json.dumps(status, ensure_ascii=False, indent=2))
raise SystemExit(0 if ok else 1)
