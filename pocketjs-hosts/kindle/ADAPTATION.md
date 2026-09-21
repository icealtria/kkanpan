# KPW3 pocketjs 适配指南

## 概览

本目录 (`pocketjs-hosts/kindle/`) 包 pocketjs 在 Kindle Paperwhite 3 上运行的 host 实现。

## 已完成

| 文件 | 状态 | 说明 |
|------|------|------|
| `Cargo.toml` | ✅ | crate 配置，依赖 pocketjs-core + pocket-ui-surface |
| `build.rs` | ✅ | 链接 vendor-fbink/libfbink.a |
| `src/main.rs` | ✅ | 事件循环 + render thread + guest 启动 |
| `src/fbink_ffi.rs` | ✅ | FBInk C API 的 Rust FFI 绑定 |
| `src/framebuffer.rs` | ✅ | RGBA8→Gray8 + 16×16 tile damage + FBInk 写入 |
| `src/input.rs` | ✅ | evdev 触摸/电源键 → pocketjs 输入格式 |
| `src/refresh.rs` | ✅ | DU/GC16 e-ink 刷新策略 |
| `platforms-patch.ts` | ✅ | platforms.ts 的 target profile 补丁 |
| `README.md` | ✅ | 构建和部署文档 |

## 已知问题

### 1. 触摸坐标溢出 ⚠️

pocketjs 的触摸格式 `(id<<18)|(y<<9)|x` 每轴只有 9 位（最大 511）。
KPW3 屏幕 1072×1448，density 2 时逻辑高度 724 > 511。

**解决方案（按推荐顺序）：**

| 方案 | 逻辑 viewport | 渲染尺寸 | 字母盒 | 说明 |
|------|-------------|---------|--------|------|
| A | 536×511 @2x | 1072×1022 | 上下各 213px | 最简单，触摸安全 |
| B | 358×483 @3x | 1074×1449 | 接近精确 | density 3，更粗的布局 |
| C | 536×724 @2x | 1072×1448 | 无 | 精确匹配，但 Y 需 clamp |

**推荐方案 A**：536×511 @ density 2，垂直方向留 213px 字母盒区域放状态栏和页码。

### 2. FBInk 线程安全

FBInk 的 `fbink_init` 不是线程安全的（设置全局变量）。
当前实现只在 render thread 初始化一次，后续调用是线程安全的。
确保不在多个线程同时调用 `fbink_print_raw_data`。

### 3. 字体渲染

pocketjs 使用 baked font atlas（编译时生成）。
对于中文，需要：
- 在 `pocket.json` 中声明需要的中文字符集
- 使用 `text.glyphs.baked` 能力（编译时 bake 中文字形）
- 或使用 `text.glyphs.runtime`（运行时加载系统字体）

Kindle 自带中文字体在 `/usr/java/lib/fonts/`，可作为 runtime fallback。

### 4. 网络

pocketjs 没有内置网络模块。kkanpan 的股票数据抓取需要在 host 层实现：
- 方案 1：用 Rust 的 `reqwest` 或 `ureq` 实现 HTTP client
- 方案 2：通过 pocketjs 的 `io.offload` 能力，用 companion 进程做网络
- 方案 3：先做离线版本，数据通过 USB 预加载

## 构建步骤

```bash
# 1. 安装工具链
rustup target add armv7-unknown-linux-gnueabi
cargo install cargo-zigbuild

# 2. 克隆 pocketjs
git clone https://github.com/pocket-stack/pocketjs
cd pocketjs
bun install

# 3. 复制 kindle host
cp -r /path/to/kkanpan/pocketjs-hosts/kindle hosts/kindle

# 4. 构建 host
cd hosts/kindle
FBINK_LIB_DIR=../../vendor-fbink/fbinklib \
  cargo zigbuild --release --target armv7-unknown-linux-gnueabi

# 5. 构建 app（以 kkanpan 为例）
cd ../../
bun pocket compile --target kindle --manifest apps/kkanpan/pocket.json

# 6. 部署到 Kindle
cp hosts/kindle/target/armv7-unknown-linux-gnueabi/release/kindle-host /mnt/us/extensions/kkanpan/
cp dist/kkanpan.js /mnt/us/extensions/kkanpan/app.js
cp dist/kkanpan.pak /mnt/us/extensions/kkanpan/app.pak
```

## 下一步

1. **解决触摸 Y 溢出**：确定 viewport 方案（推荐 536×511 @2x）
2. **移植 kkanpan app**：将 Go UI 重写为 Solid/Vue Vapor TypeScript 组件
3. **实现网络层**：Rust HTTP client 或 companion 进程
4. **Kindle 系统集成**：背光管理、共存模式（可作为 native module）
5. **实机测试**：在 KPW3 上验证渲染、触摸、刷新效果
