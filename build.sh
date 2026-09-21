#!/bin/bash
set -e

FBINK_DIR="/tmp/fbink-build"
FBINK_LIB="fbinklib/libfbink.a"
CROSS_CC="${CROSS_CC:-arm-linux-gnueabihf-gcc}"

echo "==> 检查交叉编译器: $CROSS_CC"
if ! command -v "$CROSS_CC" &>/dev/null; then
    echo "ERROR: $CROSS_CC not found."
    echo ""
    echo "Options:"
    echo "  macOS:   use Docker (see below) or install via koxtoolchain"
    echo "  Linux:   apt install gcc-arm-linux-gnueabihf"
    echo "  Docker:  export CROSS_CC=/usr/bin/arm-linux-gnueabihf-gcc"
    echo ""
    echo "Quick Docker build:"
    echo "  docker run --rm -v \$(pwd):/src -w /src shermp/fbink-build:latest bash -c './build.sh'"
    exit 1
fi

echo "==> 编译 FBInk for Kindle (PIC static library)..."
if [ ! -f "$FBINK_LIB" ]; then
    if [ ! -d "$FBINK_DIR" ]; then
        git clone --depth 1 https://github.com/NiLuJe/FBInk.git "$FBINK_DIR"
        cd "$FBINK_DIR" && git submodule update --init && cd -
    fi
    make -C "$FBINK_DIR" pic KINDLE=1 CC="$CROSS_CC" STATIC_LIBM=1
    mkdir -p fbinklib
    cp "$FBINK_DIR/Release/libfbink.a" "$FBINK_LIB"
    echo "==> FBInk library built: $FBINK_LIB"
else
    echo "==> FBInk library exists: $FBINK_LIB"
fi

echo "==> 交叉编译 kkanpan (Kindle ARM, CGO + FBInk)..."
CGO_ENABLED=1 \
GOOS=linux \
GOARCH=arm \
GOARM=7 \
CC="$CROSS_CC" \
go build -tags kindle \
    -ldflags="-s -w" \
    -o kkanpan .

mkdir -p extensions/kkanpan/bin
cp kkanpan extensions/kkanpan/bin/
cp app.json extensions/kkanpan/
cp stocks.json extensions/kkanpan/
cp config.xml extensions/kkanpan/ 2>/dev/null || true

echo "==> 编译完成: kkanpan ($(ls -lh kkanpan | awk '{print $5}'))"
echo "==> 已打包 KUAL 插件目录: extensions/kkanpan/"
