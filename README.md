# kkanpan

A stock ticker for Kindle e-readers, written in Rust.

Renders stock data as grayscale images directly to the e-ink framebuffer via [FBInk](https://github.com/NiLuJe/FBInk). Supports A-shares, US equities, and commodities with auto-switching based on trading hours.

## Features

- **E-ink optimized rendering** — SVG to grayscale pipeline with partial refresh (DU waveform) and analytic dirty rects to minimize flash and latency.
- **Touch input** — swipe vertically to flip pages, tap tabs to switch views, tap corners for style toggle and exit. Power button toggles touch on/off.
- **Auto mode** — switches stock groups automatically based on CST time rules (e.g. A-shares during 09:00-15:30, US equities after 16:00).
- **Multi-source data** — Tencent (`qt.gtimg.cn`) for A-shares/US stocks, Yahoo Finance for commodities. Parallel fetch via scoped threads.
- **HTTP server** — optional web UI for remote monitoring and control (`--http`). Exposes `/screen.png`, `/api`, `/switch`, `/style`, `/exit` endpoints.
- **Kindle coexistence** — disables Pillow/screensaver, manages frontlight, restores state on exit. Compatible with KUAL extensions.
- **Page cache** — renders all pages upfront, caches per view+style key. Partial refresh skips unchanged blocks.

## Building

Requires `cargo-zigbuild` for cross-compilation and a pre-built `fbinklib/libfbink.a` (ARM32 static library).

```sh
cargo install cargo-zigbuild
./build.sh
```

This produces a KUAL plugin at `extensions/kkanpan/` ready to deploy.

## Usage

```sh
kkanpan [OPTIONS]

Options:
  --once          Render once and exit (for use with cron/shell loop)
  --http          Start HTTP server for remote monitoring
  --port PORT     HTTP server port (default: 8000)
  --host HOST     HTTP server bind address (default: 0.0.0.0)
  --interval SEC  Data refresh interval in seconds (default: 60)
  --width W       Screen width in pixels (default: 1072)
  --height H      Screen height in pixels (default: 1448)
  --view VIEW     Initial view tab (default: from app.json)
```

### Run on Kindle

Deploy `extensions/kkanpan/` to `/mnt/us/extensions/kkanpan/` on your Kindle, then launch via KUAL or:

```sh
sh /mnt/us/extensions/kkanpan/run_on_kindle.sh
```

The shell loop runs `kkanpan -once` every 60 seconds as a coexistence mode alternative.

## Configuration

### `stocks.json`

Stock list with grouping and data source:

```json
[
  {"code": "sh000001", "name": "SSE Index", "group": "a-share", "source": "tencent"},
  {"code": "usNVDA", "name": "NVIDIA", "group": "US", "source": "tencent"},
  {"code": "GC=F", "name": "Gold", "group": "other", "source": "yahoo"}
]
```

### `app.json`

```json
{
  "proxy": "http://127.0.0.1:7890",
  "cacheTTL": 55,
  "dimFrontlight": true,
  "defaultView": "AUTO",
  "autoRules": [
    {"group": "a-share", "weekdays": [1,2,3,4,5], "start": "09:00", "end": "15:30"},
    {"group": "US", "weekdays": [1,2,3,4,5], "start": "16:00", "end": "23:59"},
    {"group": "US", "weekdays": [2,3,4,5,6], "start": "00:00", "end": "08:00"}
  ]
}
```

Config is loaded from `app.json` in the working directory, or from `/mnt/us/extensions/kkanpan/app.json` / `/mnt/us/kkanpan/app.json` on Kindle.

## Debug

Set `KKANPAN_LOG=debug` to enable verbose rendering/font/diff logs.

## Architecture

```
src/
  main.rs     — event loop, argument parsing, page cache
  config.rs   — stock/app config, auto-rule matching, CST time
  fetch.rs    — HTTP fetch from Tencent/Yahoo, in-memory cache
  render.rs   — SVG template rendering, grayscale conversion
  fbink.rs    — FBInk FFI, analytic dirty rects (zero-diffing) partial refresh
  input.rs    — touch/power button listeners, view/style state
  kindle.rs   — Kindle system integration (battery, frontlight, coexistence)
  server.rs   — optional HTTP server for remote monitoring
  util.rs     — debug logging macro
```

## License

Unlicense
