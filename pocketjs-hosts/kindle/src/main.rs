//! kindle-host — the PocketJS UI runtime on Kindle Paperwhite 3.
//!
//! Reuses the backend-agnostic `ui` surface (`pocket_ui_surface::UiSurface`)
//! and the core's software rasterizer, then blits the frame as Gray8 through
//! FBInk to the Kindle framebuffer. See docs/IMPLEMENTATION.md in this
//! directory for the full design.
//!
//! Event-loop model (mirrors hosts/pocketbook): `iv_main`-style event loop
//! on the main thread forwarding evdev events into an mpsc channel; a second
//! thread owns the FBInk framebuffer and the PocketJS tick/render loop,
//! pulling events with a timeout so it ticks on a fixed cadence even when idle.

mod fbink_ffi;
mod framebuffer;
mod input;
mod refresh;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pocket_mod::Guest;
use pocket_ui_surface::UiSurface;

use framebuffer::DirtyRect;

/// Host platform-contract identity. Must match `kindle.hostAbi` in
/// contracts/spec/platforms.ts, or plan-built bundles refuse this host.
const HOST_ID: &str = "kindle";
const HOST_ABI: u32 = 6;

/// Logical tick cadence. E-ink doesn't need 60 fps; ~30 fps keeps animations
/// smooth while sparing CPU and battery.
const TICK_MS: u64 = 33;

/// KPW3 screen: 1072×1448 @ 300 DPI
/// Logical viewport at density 2: 536×724
/// Render buffer: 536×2 = 1072, 724×2 = 1448 (exact match)
const LOGICAL_W: u32 = 536;
const LOGICAL_H: u32 = 724;
const DENSITY: u32 = 2;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!(
        "kindle-host: logical {}x{} @{}x → render {}x{}",
        LOGICAL_W,
        LOGICAL_H,
        DENSITY,
        LOGICAL_W * DENSITY,
        LOGICAL_H * DENSITY,
    );

    // Find the touchscreen evdev device
    let touch_dev = find_touch_device();
    log::info!("Touch device: {}", touch_dev.as_deref().unwrap_or("none"));

    // Find the power button evdev device
    let power_dev = find_power_device();
    log::info!("Power button device: {}", power_dev.as_deref().unwrap_or("none"));

    let (tx, rx) = mpsc::channel::<input::Event>();

    // Spawn touch listener thread
    if let Some(dev) = touch_dev {
        let tx = tx.clone();
        std::thread::spawn(move || input::touch_loop(dev, tx));
    }

    // Spawn power button listener thread
    if let Some(dev) = power_dev {
        let tx = tx.clone();
        std::thread::spawn(move || input::power_loop(dev, tx));
    }

    // Render thread: owns FBInk + guest + core + pipeline
    let render = std::thread::spawn(move || run(rx));

    // Main thread blocks until render thread exits
    render
        .join()
        .map_err(|_| anyhow::anyhow!("render thread panicked"))?
}

/// The render thread: boot the guest, then tick/render until Quit.
fn run(rx: mpsc::Receiver<input::Event>) -> Result<()> {
    // Initialize FBInk
    let fbfd = unsafe {
        let fd = fbink_ffi::fbink_open();
        if fd < 0 {
            anyhow::bail!("fbink_open failed");
        }
        let cfg = fbink_ffi::FBInkConfig {
            is_quiet: true,
            ..Default::default()
        };
        fbink_ffi::fbink_init(fd, &cfg);
        log::info!(
            "FBInk {} initialized",
            std::ffi::CStr::from_ptr(fbink_ffi::fbink_version())
                .to_string_lossy()
        );
        fd
    };

    // Boot the guest exactly like uihost: feed pak, mount ui, eval bundle.
    let pak = std::fs::read(pak_path()).with_context(|| format!("reading {}", pak_path()))?;
    let bundle =
        std::fs::read_to_string(js_path()).with_context(|| format!("reading {}", js_path()))?;

    let surface =
        UiSurface::new_with_density((LOGICAL_W as f32, LOGICAL_H as f32), DENSITY);
    surface.set_identity(HOST_ID, HOST_ABI);
    surface.feed_pak(&pak);

    let guest = Guest::new()?;
    surface.mount(&guest)?;
    guest.eval("app", &bundle)?;
    anyhow::ensure!(
        guest.has_frame(),
        "bundle installed no frame() — is this a PocketJS app?"
    );

    let render_w = (LOGICAL_W * DENSITY) as usize;
    let render_h = (LOGICAL_H * DENSITY) as usize;
    let mut fb = framebuffer::FramebufferPipeline::new(render_w, render_h);
    let mut refresh = refresh::Refresh::new();
    let mut input_state = input::Input::new(LOGICAL_W, LOGICAL_H);

    // First paint: full refresh so the screen starts clean
    tick(
        &guest,
        &surface,
        &mut fb,
        &mut refresh,
        &mut input_state,
        fbfd,
        true,
    )?;

    let mut last_tick = Instant::now();
    loop {
        // Pull events until the tick deadline, then drain any burst
        let deadline = last_tick + Duration::from_millis(TICK_MS);
        let mut quit = false;
        let mut full = false;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match rx.recv_timeout(deadline - now) {
                Ok(ev) => match input_state.on_event(ev) {
                    input::Outcome::Quit => {
                        quit = true;
                        break;
                    }
                    input::Outcome::FullRedraw => full = true,
                    input::Outcome::Continue => {}
                },
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    quit = true;
                    break;
                }
            }
        }
        while let Ok(ev) = rx.try_recv() {
            match input_state.on_event(ev) {
                input::Outcome::Quit => quit = true,
                input::Outcome::FullRedraw => full = true,
                input::Outcome::Continue => {}
            }
        }
        if quit {
            break;
        }

        last_tick = Instant::now();
        tick(
            &guest,
            &surface,
            &mut fb,
            &mut refresh,
            &mut input_state,
            fbfd,
            full,
        )?;
    }

    // Cleanup
    unsafe {
        fbink_ffi::fbink_close(fbfd);
    }
    Ok(())
}

/// One fixed-step frame: guest turn → core tick → draw → raster → gray → blit
/// → panel update.
#[allow(clippy::too_many_arguments)]
fn tick(
    guest: &Guest,
    surface: &UiSurface,
    fb: &mut framebuffer::FramebufferPipeline,
    refresh: &mut refresh::Refresh,
    input_state: &mut input::Input,
    fbfd: i32,
    full: bool,
) -> Result<()> {
    let (buttons, analog, touches) = input_state.snapshot();
    guest.frame_with_touches(buttons, analog, &touches)?;

    surface.tick();

    // Incremental raster + tile-based damage
    let dirty = surface.with_ui(|ui| {
        let words = ui.draw().words.clone();
        let plan = fb.rasterize(ui, &words);
        fb.diff(&plan)
    });

    if full {
        // Full panel redraw
        fb.blit_all(fbfd);
        refresh.full(fbfd);
        fb.advance_full();
    } else if !dirty.is_empty() {
        fb.blit_dirty(fbfd, &dirty);
        refresh.present(fbfd, &dirty);
        fb.advance(&dirty);
    } else {
        // No pixel change; let the refresh policy run its quiet cleanup
        refresh.present(fbfd, &[]);
    }
    Ok(())
}

fn pak_path() -> String {
    std::env::var("POCKET_PAK").unwrap_or_else(|_| "app.pak".into())
}

fn js_path() -> String {
    std::env::var("POCKET_JS").unwrap_or_else(|_| "app.js".into())
}

/// Scan /dev/input/event* for a touchscreen device (EV_ABS capability).
fn find_touch_device() -> Option<String> {
    for i in 0..10 {
        let path = format!("/dev/input/event{}", i);
        if std::path::Path::new(&path).exists() {
            // Heuristic: on KPW3, the touchscreen is usually event1
            // A proper implementation would check evdev capabilities
            if i == 1 || i == 0 {
                return Some(path);
            }
        }
    }
    None
}

/// Scan /dev/input/event* for the power button device.
fn find_power_device() -> Option<String> {
    for i in 0..10 {
        let path = format!("/dev/input/event{}", i);
        if std::path::Path::new(&path).exists() {
            if i == 0 || i == 2 {
                return Some(path);
            }
        }
    }
    None
}
