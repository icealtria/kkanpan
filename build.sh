#!/bin/bash
set -e

FBINK_DIR="/tmp/fbink-build"
FBINK_LIB="fbinklib/libfbink.a"
TARGET="armv7-unknown-linux-musleabihf"
OUT="target/$TARGET/release/kkanpan"

if [ ! -f "$FBINK_LIB" ]; then
    echo "==> 编译 FBInk for Kindle (PIC static library)..."
    if ! command -v arm-linux-gnueabihf-gcc &>/dev/null; then
        echo "ERROR: arm-linux-gnueabihf-gcc not found (only needed to build FBInk itself)."
        echo "Install it (Debian: apt install gcc-arm-linux-gnueabihf) or reuse the existing fbinklib/libfbink.a."
        exit 1
    fi
    if [ ! -d "$FBINK_DIR" ]; then
        git clone --depth 1 https://github.com/NiLuJe/FBInk.git "$FBINK_DIR"
        cd "$FBINK_DIR" && git submodule update --init && cd -
    fi
    make -C "$FBINK_DIR" pic KINDLE=1 CC="arm-linux-gnueabihf-gcc" STATIC_LIBM=1
    mkdir -p fbinklib
    cp "$FBINK_DIR/Release/libfbink.a" "$FBINK_LIB"
    echo "==> FBInk library built: $FBINK_LIB"
else
    echo "==> FBInk library exists: $FBINK_LIB"
fi

if ! command -v cargo-zigbuild &>/dev/null; then
    echo "ERROR: cargo-zigbuild not found. Install with: cargo install cargo-zigbuild (requires zig)"
    exit 1
fi

echo "==> 交叉编译 kkanpan (Kindle ARM32, musl static)..."
cargo zigbuild --release --target "$TARGET"

mkdir -p extensions/kkanpan/bin
cp "$OUT" extensions/kkanpan/bin/kkanpan
cp app.json extensions/kkanpan/
cp stocks.json extensions/kkanpan/

echo "==> 编译完成: $OUT ($(ls -lh "$OUT" | awk '{print $5}'))"
echo "==> 已打包 KUAL 插件目录: extensions/kkanpan/"
