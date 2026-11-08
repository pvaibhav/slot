use std::time::{Duration, Instant};

use slot::frontend::Frontend;
use slot::input::HostInput;
use slot::app::BottomSource;
use slot::screen::Screen;
use slot_gfx::{ds_rect, Compositor, DsPass, DsScreen, HostSurface, Surface};
use slot_power::SimPlatform;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

const FRAME: Duration = Duration::from_micros(16_667);

struct Slot {
    gfx: Option<(HostSurface, Compositor)>,
    frontend: Frontend,
    input: HostInput,
    ds: Option<DsPass>,
    pointer: Option<(f64, f64)>,
    pressed: bool,
}

impl Slot {
    fn new() -> Self {
        Slot {
            gfx: None,
            frontend: Frontend::boot(Box::new(SimPlatform::new())),
            input: HostInput::new(),
            ds: None,
            pointer: None,
            pressed: false,
        }
    }
}

impl ApplicationHandler for Slot {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let built = HostSurface::new(events).and_then(|s| Compositor::new(&s).map(|c| (s, c)));
        let (surface, mut compositor) = match built {
            Ok(gfx) => gfx,
            Err(e) => {
                eprintln!("slot: {e}");
                events.exit();
                return;
            }
        };
        if surface.panel_count() > 1 {
            self.ds = DsPass::new().map_err(|e| eprintln!("slot: ds pass: {e}")).ok();
            self.frontend.set_two_panels(self.ds.is_some());
        }
        self.frontend.upload_faces(&mut compositor);
        self.frontend
            .upload_bezel(&mut compositor, surface.window_size());
        self.gfx = Some((surface, compositor));
    }

    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.input.on_window_event(&event);
        let Some((surface, compositor)) = self.gfx.as_mut() else {
            return;
        };
        let on_bottom = surface.bottom_window_id() == Some(id);
        match event {
            WindowEvent::CloseRequested => events.exit(),
            WindowEvent::Resized(size) => surface.resize_window(id, size),
            WindowEvent::CursorMoved { position, .. } if on_bottom => {
                self.pointer = Some((position.x, position.y));
            }
            WindowEvent::CursorLeft { .. } if on_bottom => self.pointer = None,
            WindowEvent::MouseInput { state, button: winit::event::MouseButton::Left, .. } if on_bottom => {
                self.pressed = state.is_pressed();
            }
            WindowEvent::RedrawRequested if !on_bottom => {
                let began = Instant::now();
                if let Some(pass) = self.ds.as_mut() {
                    let touch = self.pointer.filter(|_| self.pressed).and_then(|(x, y)| {
                        let _ = surface.select_panel(1);
                        let at = touch_at(surface.window_size(), x, y);
                        let _ = surface.select_panel(0);
                        at
                    });
                    self.frontend.set_ds_touch(touch);
                    let drawn = draw_both(surface, compositor, &mut self.frontend, pass);
                    if let Err(e) = drawn {
                        eprintln!("slot: {e}");
                        events.exit();
                        return;
                    }
                }
                if self.ds.is_none() {
                    self.frontend.render(compositor, surface.window_size());
                }
                let swap = std::time::Instant::now();
                let swapped = match self.ds.is_some() {
                    true => Ok(()),
                    false => surface.swap(),
                };
                slot::latency::swapped(swap.elapsed().as_secs_f64() * 1000.0);
                if let Err(e) = swapped {
                    eprintln!("slot: {e}");
                    events.exit();
                    return;
                }
                surface.request_redraw();
                self.frontend.advance(&mut self.input);
                if self.frontend.restarting() {
                    self.frontend.restart();
                }
                if self.frontend.relaunching() {
                    self.frontend.relaunch();
                    events.exit();
                    return;
                }
                if self.frontend.repairing() {
                    self.frontend.repair();
                }
                if self.frontend.powering_off() {
                    self.frontend.poweroff();
                    events.exit();
                    return;
                }
                if let Some(left) = FRAME.checked_sub(began.elapsed()) {
                    std::thread::sleep(left);
                }
            }
            _ => {}
        }
    }
}

fn touch_at(panel: (u32, u32), x: f64, y: f64) -> Option<(u8, u8)> {
    let [rx, ry, rw, rh] = ds_rect(panel);
    let (u, v) = ((x - rx as f64) / rw as f64, (y - ry as f64) / rh as f64);
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return None;
    }
    Some(((u * 256.0) as u8, (v * 192.0) as u8))
}

fn draw_both(
    surface: &mut HostSurface,
    compositor: &mut Compositor,
    frontend: &mut Frontend,
    pass: &mut DsPass,
) -> Result<(), slot_gfx::GfxError> {
    surface.select_panel(0)?;
    frontend.render(compositor, surface.window_size());
    if let Some(frame) = frontend.ds_frame() {
        pass.set_grade(compositor.grade());
        pass.upload(frame);
    }
    surface.swap()?;
    surface.select_panel(1)?;
    let panel = surface.window_size();
    match frontend.bottom_source() {
        BottomSource::Compositor => frontend.render_screen(compositor, panel, Screen::Bottom),
        BottomSource::DsScreen if frontend.ds_frame().is_some() => {
            pass.draw_screen_powered(DsScreen::Bottom, panel, frontend.screen_power())
        }
        BottomSource::DsScreen | BottomSource::Black => pass.clear(panel),
    }
    let swapped = surface.swap();
    surface.select_panel(0)?;
    swapped
}

pub fn run() {
    let events = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("slot: {e}");
            return;
        }
    };
    events.set_control_flow(ControlFlow::Poll);
    if let Err(e) = events.run_app(&mut Slot::new()) {
        eprintln!("slot: {e}");
    }
}
