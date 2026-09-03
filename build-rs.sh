#!/bin/sh
# kkanpan Rust 版 Kindle KPW3 (ARMv7, 静态 musl) 构建脚本
#
# 一次性准备 (任选其一):
#   brew install zig && cargo install cargo-zigbuild   # 推荐, macOS 上最省事
#   # 或 Linux: apt install musl-tools gcc-arm-linux-gnueabihf
#
# 用法: ./build-rs.sh
set -e
TARGET=armv7-unknown-linux-musleabihf
ZIGBUILD="$(command -v cargo-zigbuild 2>/dev/null || echo "$HOME/.cargo/bin/cargo-zigbuild")"

if [ ! -x "$ZIGBUILD" ]; then
  echo "==> 需要 cargo-zigbuild (cargo install cargo-zigbuild, 需 zig)" >&2
  exit 1
fi
"$ZIGBUILD" zigbuild --release --target "$TARGET"

OUT="$(cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/$TARGET/release/kkanpan"

mkdir -p extensions/kkanpan/bin
cp "$OUT" extensions/kkanpan/bin/kkanpan
cp app.json extensions/kkanpan/ 2>/dev/null || true
cp stocks.json extensions/kkanpan/ 2>/dev/null || true

echo "==> Rust 构建完成: extensions/kkanpan/bin/kkanpan ($(ls -lh extensions/kkanpan/bin/kkanpan | awk '{print $5}'))"
