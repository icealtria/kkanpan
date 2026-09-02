#!/bin/sh
# 安全停止脚本 (共存模式, 不重启)
killall -9 kkanpan 2>/dev/null
sleep 1
killall -CONT awesome 2>/dev/null || true
killall -CONT cvm 2>/dev/null || true
killall -CONT volumd 2>/dev/null || true
start statusbar 2>/dev/null || true
start blanket 2>/dev/null || true
lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null || true
lipc-set-prop -i com.lab126.powerd preventScreenSaver 0 2>/dev/null || true
lipc-set-prop com.lab126.appmgrd show app://com.lab126.booklet.home 2>/dev/null || true
/usr/sbin/eips -c 2>/dev/null || true
rm -f /tmp/kkanpan.log /var/tmp/kkanpan-fb.dump 2>/dev/null || true
echo "[kkanpan] stopped and UI restored"
