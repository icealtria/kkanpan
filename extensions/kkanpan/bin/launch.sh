#!/bin/sh
# kkanpan KUAL Launch Script - 共存模式 (不杀 framework, 退出不重启, 无桌面日志)
EXT_DIR="/mnt/us/extensions/kkanpan"
BIN="${EXT_DIR}/bin/kkanpan"
LOG="/tmp/kkanpan.log"

chmod +x "${BIN}" 2>/dev/null
killall -9 kkanpan 2>/dev/null
sleep 1

echo "[kkanpan] Starting in COEXIST mode (pillow+awesome, framework kept)..."
# GOGC=300: 内存增长300%才触发GC, 用内存换CPU (省电)
# GOMEMLIMIT: 限制最大内存防OOM
env GOGC=300 GOMEMLIMIT=100MiB "${BIN}" -interval 60 > "${LOG}" 2>&1

# Go 内已 defer EnableCoexistMode, 兜底再恢复一次 (防 crash)
killall -CONT awesome 2>/dev/null || true
killall -CONT cvm 2>/dev/null || true
killall -CONT volumd 2>/dev/null || true
start statusbar 2>/dev/null || true
lipc-set-prop com.lab126.pillow disableEnablePillow enable 2>/dev/null || true
lipc-set-prop -i com.lab126.powerd preventScreenSaver 0 2>/dev/null || true
lipc-set-prop com.lab126.appmgrd show app://com.lab126.booklet.home 2>/dev/null || true
/usr/sbin/eips -c 2>/dev/null || true
# 清理临时日志与 fb dump, 避免桌面残留
rm -f /tmp/kkanpan.log /var/tmp/kkanpan-fb.dump 2>/dev/null || true
echo "[kkanpan] finished, UI restored"
