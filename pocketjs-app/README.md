# KKANPAN — PocketJS SolidJS App

kkanpan 股票看板的 pocketjs 重写版本，使用 SolidJS 组件框架。

## 项目结构

```
pocketjs-app/
├── pocket.json          # pocketjs 应用配置
├── tsconfig.json        # TypeScript 配置
└── src/
    ├── main.tsx         # 入口：mount SolidJS app
    ├── app.tsx          # 主界面组件
    ├── types.ts         # TypeScript 类型定义
    └── mock-data.ts     # 开发用 mock 数据
```

## 组件架构

```
App
├── Header          标题 + 模式标签 + 样式切换 + 退出按钮
├── TabBar          分组切换 (AUTO / a-share / US / other / ALL)
├── StockList       股票卡片列表 (分页)
│   └── StockCard   单只股票卡片
│       ├── 名称 + 代码
│       ├── Sparkline  迷你图表 (View 元素绘制的柱状图)
│       └── 价格 + 涨跌幅
└── Footer          页码指示器 + 状态栏
```

## 技术要点

### Sparkline 实现

pocketjs 没有 SVG/canvas，sparkline 使用 `<View>` 元素实现：
- 每个价格点是一个竖条 (`View` with height)
- 高度按价格比例缩放
- 横向排列形成柱状图效果

### 样式系统

使用 pocketjs 的 Tailwind 子集：
- 文字大小：`text-xs`(12px), `text-sm`(14px), `text-base`(16px), `text-lg`(18px)
- 颜色：`text-slate-950`, `bg-white`, `bg-slate-900` 等
- 布局：`flex-row`, `flex-col`, `flex-1`, `gap-N`, `px-N`, `py-N`
- 交互：`focusable`, `onPress`, `focus:bg-*`, `active:bg-*`

### 数据流

当前使用 mock 数据。后续通过以下方式获取实时数据：
1. pocketjs `net.http` 能力 (host 实现 HTTP)
2. companion 进程通过共享内存传递数据
3. USB 预加载离线数据

## 构建

从 pocketjs 仓库根目录：

```bash
# 安装 pocketjs CLI
npm install -g @pocketjs/cli

# 构建 app
pocket compile --target kindle --manifest pocket.json

# 输出:
#   dist/kkanpan-main.js
#   dist/kkanpan-main.pak
```

## 开发

Mock 数据在 `src/mock-data.ts`，可直接修改添加更多股票。

组件修改后重新构建即可，pocketjs 的增量编译很快。

## 从 Go 版本迁移

| Go 版本 | pocketjs 版本 |
|---------|-------------|
| `render.go` (手绘像素) | `app.tsx` (SolidJS 组件) |
| `layout.go` (手动布局) | Tailwind 类 (flexbox) |
| `font.go` (TrueType 渲染) | baked font atlas (编译时) |
| `fbink.go` (FBInk 写屏) | `hosts/kindle` (Rust host) |
| `touch.go` (evdev 读取) | `hosts/kindle/src/input.rs` |
| `fetch.go` (HTTP 抓取) | 待实现 (net module) |
| `sysinfo.go` (系统信息) | 待实现 (native module) |
