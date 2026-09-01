#!/bin/sh
# kkanpan KUAL Launch Script (免杀 framework 完美共存 + 触屏独占模式)

EXT_DIR="/mnt/us/extensions/kkanpan"
BIN="${EXT_DIR}/bin/kkanpan"
LOG="${EXT_DIR}/app.log"

chmod +x "${BIN}" "${EXT_DIR}/bin/stop.sh" 2>/dev/null

# 1. 禁用休眠防息屏
lipc-set-prop -i com.lab126.powerd preventScreenSaver 1 2>/dev/null || true

# 2. 暂停 Kindle Java 桌面进程 (SIGSTOP，不杀死但彻底冻结其响应，省CPU且防止背景响应触控)
killall -STOP cvm 2>/dev/null || true

# 3. 清理旧看盘进程
killall -9 kkanpan 2>/dev/null

# 4. 运行 kkanpan
"${BIN}" -interval 60 > "${LOG}" 2>&1

# 5. 退出后恢复 Java 虚拟机并唤醒桌面
killall -CONT cvm 2>/dev/null || true
lipc-set-prop -i com.lab126.powerd preventScreenSaver 0 2>/dev/null || true
lipc-set-prop com.lab126.appmgrd show app://com.lab126.booklet.home 2>/dev/null || true
/usr/sbin/eips -c 2>/dev/null || true
