#!/usr/bin/env python3
import subprocess, re, time, pathlib, xml.etree.ElementTree as ET
ADB='100.127.23.27:1234'
OUT=pathlib.Path('reports/android-mvp-screenshots'); OUT.mkdir(parents=True, exist_ok=True)
LOG=pathlib.Path('reports/android-mvp-logs/e2e-ui-navigation.log'); LOG.parent.mkdir(parents=True, exist_ok=True)

def sh(cmd):
    return subprocess.check_output(cmd, shell=True, text=True, stderr=subprocess.STDOUT)

def dump_xml():
    sh(f'adb -s {ADB} shell uiautomator dump /sdcard/view.xml >/dev/null')
    sh(f'adb -s {ADB} pull /sdcard/view.xml /tmp/view.xml >/dev/null')
    return ET.parse('/tmp/view.xml').getroot()

def find_center(text):
    root=dump_xml()
    for n in root.iter('node'):
        if n.attrib.get('text')==text or n.attrib.get('content-desc')==text:
            b=n.attrib.get('bounds','')
            m=re.match(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', b)
            if m:
                x=(int(m.group(1))+int(m.group(3)))//2
                y=(int(m.group(2))+int(m.group(4)))//2
                return x,y
    return None

def click_text(text):
    xy=find_center(text)
    if not xy: return False
    sh(f'adb -s {ADB} shell input tap {xy[0]} {xy[1]}')
    time.sleep(1)
    return True

def has_text(text):
    root=dump_xml()
    for n in root.iter('node'):
        t=n.attrib.get('text','')
        c=n.attrib.get('content-desc','')
        if text in t or text in c:
            return True
    return False

def shot(name):
    sh(f'adb -s {ADB} exec-out screencap -p > {OUT/name}')

steps=[]
sh(f'adb -s {ADB} shell am start -n com.fin.client/.MainActivity >/dev/null')
time.sleep(1)
shot('e2e-ui-nav-01-home.png')
steps.append('home_started=true')

plan=[('会话','输入队列','e2e-ui-nav-02-conversation.png'),('状态','Daemon','e2e-ui-nav-03-runtime.png'),('连接','连接状态','e2e-ui-nav-04-connection.png'),('更新','检查更新','e2e-ui-nav-05-update.png')]
for tab,marker,img in plan:
    ok_click=click_text(tab)
    ok_marker=has_text(marker)
    shot(img)
    steps.append(f'tab={tab} click={ok_click} marker={ok_marker}')

# back flow
ok_back=click_text('‹')
ok_home=has_text('全部任务')
shot('e2e-ui-nav-06-back-home.png')
steps.append(f'back_click={ok_back} back_home={ok_home}')

# update check action
click_text('更新')
ok_check_btn=click_text('检查更新')
time.sleep(1)
ok_update_data=has_text('versionName') or has_text('error')
shot('e2e-ui-nav-07-update-after-check.png')
steps.append(f'update_check_click={ok_check_btn} update_result_visible={ok_update_data}')

LOG.write_text('\n'.join(steps)+'\n')
print(LOG.read_text())
