#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Btn, InputSource, Millis, RawEvent, POWER_HOLD_MS};
use slot_power::{Battery, Charge, LedState, Platform, SimPlatform};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

struct RecordingPanel {
    inner: SimPlatform,
    backlight: Arc<AtomicU8>,
}

impl Platform for RecordingPanel {
    fn set_backlight(&mut self, step: u8) {
        self.backlight.store(step, Ordering::Relaxed);
    }

    fn battery(&self) -> Option<Battery> {
        self.inner.battery()
    }

    fn charge(&self) -> Charge {
        self.inner.charge()
    }

    fn set_led(&mut self, state: LedState) {
        self.inner.set_led(state);
    }

    fn poweroff(&mut self) -> ! {
        self.inner.poweroff()
    }

    fn restart(&mut self) -> ! {
        self.inner.restart()
    }

    fn root(&self) -> &Path {
        self.inner.root()
    }

    fn now(&self) -> i64 {
        self.inner.now()
    }

    fn set_clock(&mut self, secs: i64) {
        self.inner.set_clock(secs);
    }

    fn set_rumble(&mut self, strength: u16) {
        self.inner.set_rumble(strength);
    }
}

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn all_black(px: &[u8]) -> bool {
    (0..OUT_H as usize).step_by(7).all(|y| {
        (0..OUT_W as usize)
            .step_by(7)
            .all(|x| at(px, x, y) == [0; 3])
    })
}

fn any_ink(px: &[u8]) -> bool {
    (0..OUT_H as usize).step_by(3).any(|y| {
        (0..OUT_W as usize)
            .step_by(3)
            .any(|x| at(px, x, y)[0] > 0x80)
    })
}

fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/power-wake-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

#[test]
fn power_on_a_dozing_device_brings_the_screen_back_before_the_menu() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let backlight = Arc::new(AtomicU8::new(0));
    let mut f = Frontend::boot(Box::new(RecordingPanel {
        inner: SimPlatform::at(d.path().to_path_buf()),
        backlight: backlight.clone(),
    }));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    f.advance(&mut input);

    let shelf = composed(&mut f, &mut c, "shelf");
    let lit = backlight.load(Ordering::Relaxed);
    assert!(lit > 0, "the panel never came on");
    assert!(
        any_ink(&shelf),
        "the shelf drew nothing, so going dark proves nothing"
    );

    tap(&mut f, &mut input, Btn::Power);
    let dozing = composed(&mut f, &mut c, "doze");
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        0,
        "the doze left the panel lit"
    );
    assert!(all_black(&dozing), "the doze left something on the screen");

    // And POWER pressed again. The press, held: the screen has to come back before shutdown
    // starts, not a second after it.
    input.0.push_back(vec![RawEvent::Down(Btn::Power)]);
    f.advance(&mut input);
    let woken = composed(&mut f, &mut c, "woken");
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        lit,
        "the panel is still dark under the thumb trying to wake it"
    );
    assert!(
        !all_black(&woken) && any_ink(&woken),
        "the screen is still the doze's own black"
    );

    let until = Instant::now() + Duration::from_millis(POWER_HOLD_MS + 200);
    while Instant::now() < until {
        f.advance(&mut input);
        std::thread::sleep(Duration::from_millis(8));
    }
    let shutdown = composed(&mut f, &mut c, "shutdown");
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        lit,
        "the shutdown message is on a panel nobody can see"
    );
    assert!(
        any_ink(&shutdown),
        "the shutdown message is not on the panel to be read"
    );
    assert_ne!(
        shutdown, woken,
        "the hold never showed shutdown over what was underneath it"
    );
}
