# kindle-host

The PocketJS UI runtime on **Kindle Paperwhite 3** (and compatible Kindle devices),
rendered through [FBInk](https://github.com/NiLuJe/FBInk).

It reuses the backend-agnostic `ui` surface (`pocket-ui-surface`) and the core's
software rasterizer unchanged, then:

- rasterizes the DrawList **incrementally** to a retained RGBA8 buffer at
  `536×724 @2x` = 1072×1448 (`pocketjs_core::raster::render_scaled_incremental`
  with a core `DamageTracker`), matching the `kindle` target profile in
  `contracts/spec/platforms.ts`;
- converts RGBA8 → Gray8 via luminance and pixel-diffs 16×16 tiles **inside
  the damage regions**, then blits the changed pixels through FBInk's
  `fbink_print_raw_data` as Y8 data;
- drives the panel with a DU (partial) / GC16 (full) refresh policy adapted
  from the PocketBook host;
- maps Linux evdev touch events → the framework's packed touch wire format
  (`(id<<18)|(y<<9)|x`), dividing physical coordinates by density to get
  logical viewport coordinates;
- reads the power button from a separate evdev device.

## Architecture

```
app.tsx (Solid / Vue Vapor + Tailwind)
   │  bun tools/build.ts <app> --target kindle
   ▼
app.js  +  app.pak
   │
   ▼  (loaded by the host at startup)
┌───────────────────────────────────────────────────────┐
│ QuickJS guest (pocket_mod::Guest)                      │
│   globalThis.ui  ← UiSurface (REUSED)                  │
│   globalThis.frame(buttons, analog, touches)           │
└───────────────────────────┬───────────────────────────┘
                            │ ui.* ops
                            ▼
┌───────────────────────────────────────────────────────┐
│ pocketjs_core::Ui  (inside UiSurface)                  │
│   feed_pak → load_styles / load_font_atlas             │
│   tick() → draw() → DrawList { words: Vec<u32> }      │
└───────────────────────────┬───────────────────────────┘
                            │ raster::render_scaled(ui, words, fb, 2)
                            ▼  RGBA8 @ 1072×1448
┌───────────────────────────────────────────────────────┐
│ hosts/kindle  (this crate)                             │
│   framebuffer.rs : RGBA8 → Gray8 + tile damage         │
│   refresh.rs     : DU/GC16 e-ink refresh policy        │
│   input.rs       : evdev → BTN bitmask + packed touch  │
│   main.rs        : channel event loop + render thread  │
└───────────────────────────┬───────────────────────────┘
                            │ fbink_print_raw_data + fbink_refresh
                            ▼
                  FBInk → /dev/fb0 → Kindle e-ink panel
```

## Kindle Paperwhite 3 specs

| Property | Value |
|----------|-------|
| Screen | 1072×1448 @ 300 DPI |
| Logical viewport | 536×724 (exact 2× match) |
| Render buffer | 1072×1448 (logical × density) |
| Touch | Single-touch, evdev MT protocol |
| Framebuffer | /dev/fb0 (8bpp or 32bpp, FBInk handles conversion) |
| CPU | ARM Cortex-A9 @ 1 GHz |
| RAM | 512 MB |

## Viewport design

The touch wire format packs coordinates into 9 bits per axis (max 511).
The KPW3 screen is 1072×1448, so we use:

- **Logical viewport**: 536×724 (both ≤ 511? NO — 724 > 511!)

**⚠️ IMPORTANT**: 724 > 511, which means touch Y coordinates would overflow
the 9-bit wire format. There are two solutions:

1. **Use a smaller logical viewport** (e.g., 536×511 @ density 2 = 1072×1022)
   with letterboxing on the 1448-tall screen.
2. **Increase density** (e.g., density 3: logical 358×483 → 1072×1449, close
   but not exact).

The current code uses 536×724 @ density 2 which is an exact pixel match but
will need the touch Y clamped to 511. For production, a tuned viewport like
**536×511 @ density 2** with vertical letterboxing is recommended.

## Build

One-time toolchain setup:

```sh
rustup target add armv7-unknown-linux-gnueabi
cargo install cargo-zigbuild
# zig (brew install zig) and libclang (for rquickjs bindgen) are also required
```

Cross-compile the host (from this directory):

```sh
cargo zigbuild --release --target armv7-unknown-linux-gnueabi
# → target/armv7-unknown-linux-gnueabi/release/kindle-host
```

Or with the kkanpan vendor FBInk:

```sh
FBINK_LIB_DIR=../../vendor-fbink/fbinklib \
  cargo zigbuild --release --target armv7-unknown-linux-gnueabi
```

## Build the app bundle

From the pocketjs repo root:

```sh
bun pocket compile --target kindle --manifest apps/my-app/pocket.json --project-root .
# → dist/my-app.js + dist/my-app.pak
```

## Deploy

Connect the Kindle over USB, then:

```sh
# Mount Kindle USB storage
D=/mnt/us/extensions/kkanpan
cp target/armv7-unknown-linux-gnueabi/release/kindle-host $D/kindle-host
cp dist/my-app.js $D/app.js
cp dist/my-app.pak $D/app.pak
chmod +x $D/kindle-host
```

## Runtime configuration

| Env var | Default | Meaning |
|---------|---------|---------|
| `POCKET_PAK` | `app.pak` | path to the app pak |
| `POCKET_JS` | `app.js` | path to the JS bundle |
| `RUST_LOG` | `info` | log filter |

## FBInk waveform modes

| Mode | Quality | Speed | Use case |
|------|---------|-------|----------|
| DU | Low | Fast | Partial updates, animations |
| GC16 | High | Slow | Full redraws, clean text |
| A2 | Very low | Very fast | Fast animations (2-level) |

The refresh policy automatically selects:
- **DU** for incremental partial updates (idle panel)
- **GC16** for full redraws and cleanup after ghosting
