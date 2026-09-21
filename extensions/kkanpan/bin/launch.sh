#!/bin/sh
# kkanpan KUAL Launch Script - 共存模式 (不杀 framework, 退出不重启, 无桌面日志)
EXT_DIR="/mnt/us/extensions/kkanpan"
BIN="${EXT_DIR}/bin/kkanpan"
LOG="${EXT_DIR}/kkanpan.log"

chmod +x "${BIN}" 2>/dev/null
killall -9 kkanpan 2>/dev/null
sleep 1

echo "[kkanpan] Starting in COEXIST mode (pillow+awesome, framework kept)..."
"${BIN}" -interval 60 > "${LOG}" 2>&1
killall -CONT awesome 2>/dev/null || true
killall -CONT cvm 2>/dev/null || true
killall -CONT volumd 2>/dev/null || true
start statusbar 2>/dev/null || true
lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null || true
lipc-set-prop -i com.lab126.powerd preventScreenSaver 0 2>/dev/null || true
lipc-set-prop com.lab126.appmgrd show app://com.lab126.booklet.home 2>/dev/null || true
/usr/sbin/eips -c 2>/dev/null || true
# 清理 fb dump, 避免桌面残留 (日志保留在 extensions 目录)
rm -f /var/tmp/kkanpan-fb.dump 2>/dev/null || true
echo "[kkanpan] finished, UI restored"
