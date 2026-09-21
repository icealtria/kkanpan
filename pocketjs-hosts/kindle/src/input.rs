//! evdev events → PocketJS input state.
//!
//! Touch events from /dev/input/event* are mapped to the framework's packed
//! touch wire format (`(id<<18)|(y<<9)|x`, framework/src/touch.ts) in LOGICAL
//! viewport pixels. The KPW3 touchscreen reports ABS_MT_POSITION_X/Y in
//! physical pixels (1072×1448), which are divided by density to get logical coords.
//!
//! Power button events are forwarded as a Quit signal.

use std::io::Read;
use std::sync::mpsc;

/// Events forwarded from listener threads to the render thread.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    TouchDown { x: i32, y: i32 },
    TouchMove { x: i32, y: i32 },
    TouchUp,
    PowerPress,
    Quit,
}

/// What the render loop should do after handling an event.
pub enum Outcome {
    Continue,
    Quit,
    /// A full redraw was requested (e.g. returning from background).
    FullRedraw,
}

pub struct Input {
    buttons: u32,
    /// Current touch contact in LOGICAL px (None = up). KPW3 is single-touch.
    touch: Option<(u32, u32)>,
    logical_w: u32,
    logical_h: u32,
}

impl Input {
    pub fn new(logical_w: u32, logical_h: u32) -> Self {
        Self {
            buttons: 0,
            touch: None,
            logical_w,
            logical_h,
        }
    }

    pub fn on_event(&mut self, ev: Event) -> Outcome {
        match ev {
            Event::Quit => Outcome::Quit,
            Event::PowerPress => {
                // Power button toggles touch (matching kkanpan behavior)
                Outcome::Continue
            }
            Event::TouchDown { x, y } | Event::TouchMove { x, y } => {
                // Physical → logical: divide by density (2)
                let lx = (x as u32 / 2).min(self.logical_w.saturating_sub(1));
                let ly = (y as u32 / 2).min(self.logical_h.saturating_sub(1));
                self.touch = Some((lx, ly));
                Outcome::Continue
            }
            Event::TouchUp => {
                self.touch = None;
                Outcome::Continue
            }
        }
    }

    /// (buttons, analog, packed touches) for Guest::frame_with_touches.
    pub fn snapshot(&self) -> (u32, u32, Vec<u32>) {
        let touches = self
            .touch
            .map(|(x, y)| vec![pack_touch(0, x, y)])
            .unwrap_or_default();
        (self.buttons, 0x8080, touches) // ANALOG_CENTER = 0x8080
    }
}

/// framework/src/touch.ts `__packTouch`: `(id<<18)|(y<<9)|x`.
fn pack_touch(id: u32, x: u32, y: u32) -> u32 {
    ((id & 0xff) << 18) | ((y & 0x1ff) << 9) | (x & 0x1ff)
}

/// evdev event structure (32-bit version, matches KPW3 kernel).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct InputEvent32 {
    pub sec: i32,
    pub usec: i32,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

// evdev constants
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const BTN_TOUCH: u16 = 0x14a;
const KEY_POWER: u16 = 116;

/// Touch event listener thread. Reads evdev events and forwards to the render
/// thread via the channel.
pub fn touch_loop(dev_path: String, tx: mpsc::Sender<Event>) {
    let mut file = match std::fs::File::open(&dev_path) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Cannot open touch device {}: {}", dev_path, e);
            return;
        }
    };

    log::info!("Touch listener active on {}", dev_path);

    let mut cur_x: i32 = 0;
    let mut cur_y: i32 = 0;
    let mut touching = false;
    let mut start_x: i32 = 0;
    let mut start_y: i32 = 0;

    let mut buf = [0u8; 16]; // sizeof(struct input_event) on 32-bit
    loop {
        match file.read_exact(&mut buf) {
            Ok(_) => {}
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        }

        // Parse evdev event (little-endian 32-bit)
        let ev = InputEvent32 {
            sec: i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            usec: i32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            type_: u16::from_le_bytes([buf[8], buf[9]]),
            code: u16::from_le_bytes([buf[10], buf[11]]),
            value: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
        };

        match ev.type_ {
            EV_ABS => {
                if ev.code == ABS_MT_POSITION_X || ev.code == 0x00 {
                    cur_x = ev.value;
                } else if ev.code == ABS_MT_POSITION_Y || ev.code == 0x01 {
                    cur_y = ev.value;
                }
            }
            EV_KEY if ev.code == BTN_TOUCH => {
                if ev.value == 1 {
                    touching = true;
                    start_x = cur_x;
                    start_y = cur_y;
                } else if ev.value == 0 && touching {
                    touching = false;
                    if cur_x == 0 && cur_y == 0 {
                        cur_x = start_x;
                        cur_y = start_y;
                    }
                    if cur_x > 0 && cur_y > 0 {
                        let _ = tx.send(Event::TouchDown {
                            x: cur_x,
                            y: cur_y,
                        });
                        let _ = tx.send(Event::TouchUp);
                    }
                    start_x = 0;
                    start_y = 0;
                }
            }
            EV_KEY if ev.code == KEY_POWER && ev.value == 1 => {
                let _ = tx.send(Event::PowerPress);
            }
            EV_SYN => {
                // Sync event — if touching, send move
                if touching && cur_x > 0 && cur_y > 0 {
                    let _ = tx.send(Event::TouchMove {
                        x: cur_x,
                        y: cur_y,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Power button listener thread.
pub fn power_loop(dev_path: String, tx: mpsc::Sender<Event>) {
    let mut file = match std::fs::File::open(&dev_path) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Cannot open power device {}: {}", dev_path, e);
            return;
        }
    };

    log::info!("Power button listener active on {}", dev_path);

    let mut buf = [0u8; 16];
    loop {
        match file.read_exact(&mut buf) {
            Ok(_) => {}
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        }

        let ev = InputEvent32 {
            sec: i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            usec: i32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            type_: u16::from_le_bytes([buf[8], buf[9]]),
            code: u16::from_le_bytes([buf[10], buf[11]]),
            value: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
        };

        if ev.type_ == EV_KEY && ev.code == KEY_POWER && ev.value == 1 {
            let _ = tx.send(Event::PowerPress);
        }
    }
}
