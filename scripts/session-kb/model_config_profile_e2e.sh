#!/usr/bin/env bash
set -euo pipefail
SER=100.127.23.27:1234
TS=$(date +%F-%H%M%S)
LOG=reports/session-kb-logs/model-config-host-profile-e2e-$TS.log
mkdir -p reports/session-kb-logs reports/session-kb-screenshots
exec > >(tee -a "$LOG") 2>&1

adb -s $SER shell cmd statusbar collapse || true
adb -s $SER shell service call statusbar 2 || true
adb -s $SER shell input keyevent 82 || true
adb -s $SER shell input swipe 600 2200 600 300 || true
adb -s $SER shell am start -n com.fin.client/.MainActivity
sleep 4

# open settings
adb -s $SER shell input tap 80 190
sleep 2
# capture initial profile UI
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-profile-initial.png
adb -s $SER pull /sdcard/Download/model-config-host-profile-initial.png reports/session-kb-screenshots/model-config-host-profile-initial.png >/dev/null

# open profile select and choose another option if available by key nav
adb -s $SER shell input tap 600 2285
sleep 1
adb -s $SER shell input keyevent 20 || true
adb -s $SER shell input keyevent 66 || true
sleep 1
# test selected profile
adb -s $SER shell input tap 170 2550
sleep 3
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-pass-save-enabled-profile.png
adb -s $SER pull /sdcard/Download/model-config-host-pass-save-enabled-profile.png reports/session-kb-screenshots/model-config-host-pass-save-enabled-profile.png >/dev/null

# change thinking to invalidate save
adb -s $SER shell input tap 600 2415
sleep 1
adb -s $SER shell input keyevent 20 || true
adb -s $SER shell input keyevent 66 || true
sleep 1
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-drift-save-disabled-profile.png
adb -s $SER pull /sdcard/Download/model-config-host-drift-save-disabled-profile.png reports/session-kb-screenshots/model-config-host-drift-save-disabled-profile.png >/dev/null

# re-test after drift
adb -s $SER shell input tap 170 2550
sleep 3
# save to host
adb -s $SER shell input tap 500 2550
sleep 2
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-save-success-readonly-profile.png
adb -s $SER pull /sdcard/Download/model-config-host-save-success-readonly-profile.png reports/session-kb-screenshots/model-config-host-save-success-readonly-profile.png >/dev/null

# close settings and trigger send for visible error / render path
adb -s $SER shell input tap 70 190
sleep 1
adb -s $SER shell input tap 540 2470
adb -s $SER shell input text profile_test_msg
adb -s $SER shell input tap 1120 2470
sleep 5
adb -s $SER shell screencap -p /sdcard/Download/model-config-host-502-render-profile.png
adb -s $SER pull /sdcard/Download/model-config-host-502-render-profile.png reports/session-kb-screenshots/model-config-host-502-render-profile.png >/dev/null

adb -s $SER shell run-as com.fin.client cat files/logs/connection-events.log > reports/session-kb-logs/model-config-host-profile-connection-events.log || true
adb -s $SER shell uiautomator dump /sdcard/Download/model-config-host-ui.xml >/dev/null || true
adb -s $SER pull /sdcard/Download/model-config-host-ui.xml reports/session-kb-logs/model-config-host-ui-$TS.xml >/dev/null || true

echo DONE
