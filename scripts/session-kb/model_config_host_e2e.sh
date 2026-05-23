#!/usr/bin/env bash
set -euo pipefail
SER=100.127.23.27:1234
LOG=reports/session-kb-logs/model-config-host-e2e-$(date +%F-%H%M%S).log
mkdir -p reports/session-kb-logs reports/session-kb-screenshots
exec > >(tee -a "$LOG") 2>&1

adb -s $SER shell cmd statusbar collapse || true
adb -s $SER shell service call statusbar 2 || true
adb -s $SER shell input keyevent 82 || true
adb -s $SER shell input swipe 600 2200 600 300 || true
adb -s $SER shell am start -n com.fin.client/.MainActivity
sleep 3
# open settings top-left
adb -s $SER shell input tap 80 190
sleep 1
# fill FAIL case provider/model
adb -s $SER shell input tap 260 2270
adb -s $SER shell input keyevent CTRL_A || true
adb -s $SER shell input text bad_provider
adb -s $SER shell input tap 260 2390
adb -s $SER shell input text bad_model
# click test
adb -s $SER shell input tap 170 2550
sleep 2
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-fail-save-disabled.png
adb -s $SER pull /sdcard/Download/model-config-host-fail-save-disabled.png reports/session-kb-screenshots/model-config-host-fail-save-disabled.png

# pass case using likely provider/model
adb -s $SER shell input tap 260 2270
adb -s $SER shell input text openai
adb -s $SER shell input tap 260 2390
adb -s $SER shell input text gpt-5
adb -s $SER shell input tap 170 2550
sleep 2
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-pass-save-enabled.png
adb -s $SER pull /sdcard/Download/model-config-host-pass-save-enabled.png reports/session-kb-screenshots/model-config-host-pass-save-enabled.png
# click save button right
adb -s $SER shell input tap 500 2550
sleep 2
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-save-success-readonly.png
adb -s $SER pull /sdcard/Download/model-config-host-save-success-readonly.png reports/session-kb-screenshots/model-config-host-save-success-readonly.png

# close settings
adb -s $SER shell input tap 70 190
sleep 1
# trigger a normal send to capture potential error rendering
adb -s $SER shell input tap 530 2470
adb -s $SER shell input text test_502
adb -s $SER shell input tap 1120 2470
sleep 4
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-502-render.png
adb -s $SER pull /sdcard/Download/model-config-host-502-render.png reports/session-kb-screenshots/model-config-host-502-render.png

adb -s $SER shell run-as com.fin.client cat files/logs/connection-events.log > reports/session-kb-logs/model-config-host-connection-events.log || true

echo DONE
