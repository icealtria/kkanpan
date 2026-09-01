#!/bin/bash
echo "==> 正在交叉编译适用于 Kindle Paperwhite 3 (ARMv7) 的二进制文件..."
CGO_ENABLED=0 GOOS=linux GOARCH=arm GOARM=7 go build -ldflags="-s -w" -o kkanpan .

mkdir -p extensions/kkanpan/bin
cp kkanpan extensions/kkanpan/bin/
cp config.json extensions/kkanpan/
cp config.xml extensions/kkanpan/ 2>/dev/null || true

echo "==> 编译完成: kkanpan ($(ls -lh kkanpan | awk '{print $5}'))"
echo "==> 已打包 KUAL 插件目录: extensions/kkanpan/"
echo "==> 包含 config.xml, menu.json, bin/kkanpan"
