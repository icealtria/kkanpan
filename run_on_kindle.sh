#!/bin/sh
# kkanpan 启动与低功耗循环脚本 (运行于 Kindle 内部)

BIN="/mnt/us/kkanpan/kkanpan"
CONFIG="/mnt/us/kkanpan/config.json"
INTERVAL=60 # 刷新周期 (秒)

if [ ! -f "$BIN" ]; then
    echo "未找到可执行文件 $BIN"
    exit 1
fi
chmod +x "$BIN"

echo "1. 停止 Kindle 前台界面 (释放屏幕)..."
stop framework 2>/dev/null || true
sleep 1

echo "2. 启动看盘循环..."
while true; do
    # 调用单次执行模式刷新墨水屏
    $BIN -once
    
    # 睡眠等待下一次刷新 (若系统支持 rtcwake 可深度休眠省电)
    sleep $INTERVAL
done
