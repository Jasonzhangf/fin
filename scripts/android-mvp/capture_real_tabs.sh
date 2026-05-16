#!/usr/bin/env bash
set -euo pipefail
ADB=${1:-100.127.23.27:1234}
OUT=reports/android-mvp-screenshots
mkdir -p "$OUT"

# dump xml and tap center of node containing text
click_text(){
  local text="$1"
  adb -s "$ADB" shell uiautomator dump /sdcard/view.xml >/dev/null
  adb -s "$ADB" pull /sdcard/view.xml /tmp/view.xml >/dev/null
  python3 - "$text" <<'PY'
import re,sys,xml.etree.ElementTree as ET
text=sys.argv[1]
root=ET.parse('/tmp/view.xml').getroot()
for n in root.iter('node'):
    t=n.attrib.get('text','')
    c=n.attrib.get('content-desc','')
    if t==text or c==text:
        b=n.attrib.get('bounds','')
        m=re.match(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', b)
        if not m: continue
        x=(int(m.group(1))+int(m.group(3)))//2
        y=(int(m.group(2))+int(m.group(4)))//2
        print(f"{x} {y}")
        sys.exit(0)
print('')
PY
}

shot(){ adb -s "$ADB" exec-out screencap -p > "$1"; }

adb -s "$ADB" shell am start -n com.fin.client/.MainActivity >/dev/null
sleep 1
shot "$OUT/e2e-ui-01-home.png"

for tab in 会话 状态 连接 更新 任务; do
  XY=$(click_text "$tab" || true)
  if [ -n "$XY" ]; then
    adb -s "$ADB" shell input tap $XY
    sleep 1
    case "$tab" in
      会话) shot "$OUT/e2e-ui-02-conversation.png";;
      状态) shot "$OUT/e2e-ui-03-runtime.png";;
      连接) shot "$OUT/e2e-ui-04-connection.png";;
      更新) shot "$OUT/e2e-ui-05-update.png";;
      任务) shot "$OUT/e2e-ui-06-sessions-back.png";;
    esac
  fi
done

# click update check button if visible
XY=$(click_text "检查更新" || true)
if [ -n "$XY" ]; then
  adb -s "$ADB" shell input tap $XY
  sleep 1
  shot "$OUT/e2e-ui-07-update-after-check.png"
fi

echo "captured to $OUT"
