#!/bin/sh
# kkanpan 启动循环脚本 - 共存模式 (不杀 framework, 无重启)
BIN="/mnt/us/kkanpan/kkanpan"
INTERVAL=60
if [ ! -f "$BIN" ]; then echo "未找到 $BIN"; exit 1; fi
chmod +x "$BIN"
echo "启动看盘循环 (共存模式)..."
while true; do
    $BIN -once
    sleep $INTERVAL
done
