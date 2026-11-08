use crate::surface::{GfxError, Surface};
use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext, PossiblyCurrentContext,
    PossiblyCurrentGlContext, Version,
};
use glutin::display::{Display, GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use std::ffi::{c_void, CString};
use std::num::NonZeroU32;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

const DEFAULT_W: u32 = 1440;
const DEFAULT_H: u32 = 960;

const DUAL_W: u32 = 1024;
const DUAL_H: u32 = 768;

type GlWindowSurface = glutin::surface::Surface<WindowSurface>;

pub struct HostSurface {
    window: Window,
    display: Display,
    gl_surface: GlWindowSurface,
    context: PossiblyCurrentContext,
    bottom: Option<(Window, GlWindowSurface)>,
    current: usize,
}

fn err<E: std::fmt::Display>(e: E) -> GfxError {
    GfxError::Context(e.to_string())
}

fn size() -> (u32, u32) {
    let fallback = match dual() {
        true => (DUAL_W, DUAL_H),
        false => (DEFAULT_W, DEFAULT_H),
    };
    std::env::var("SLOT_WINDOW")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or(fallback)
}

pub fn dual() -> bool {
    std::env::var_os("SLOT_DUAL").is_some_and(|v| v != "0")
}

fn bare() -> bool {
    std::env::var_os("SLOT_BARE").is_some_and(|v| v != "0")
}

impl HostSurface {
    pub fn new(events: &ActiveEventLoop) -> Result<Self, GfxError> {
        let mut attrs = Window::default_attributes()
            .with_title("slot.")
            .with_inner_size(winit::dpi::PhysicalSize::<u32>::from(size()));
        if bare() {
            attrs = attrs.with_decorations(false);
        }
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attrs))
            .build(events, ConfigTemplateBuilder::new(), pick_config)
            .map_err(err)?;
        let window = window.ok_or_else(|| GfxError::Context("no window was created".into()))?;

        let display = config.display();
        let handle = window.window_handle().map_err(err)?.as_raw();
        let context_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .with_profile(GlProfile::Core)
            .build(Some(handle));
        let context = unsafe { display.create_context(&config, &context_attrs) }.map_err(err)?;

        let surface_attrs = window
            .build_surface_attributes(Default::default())
            .map_err(err)?;
        let gl_surface =
            unsafe { display.create_window_surface(&config, &surface_attrs) }.map_err(err)?;
        let context = context.make_current(&gl_surface).map_err(err)?;

        let _ = gl_surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN));

        let bottom = match dual() {
            true => Some(bottom_window(events, &window, &display, &config, &context)?),
            false => None,
        };
        context.make_current(&gl_surface).map_err(err)?;

        Ok(HostSurface {
            window,
            display,
            gl_surface,
            context,
            bottom,
            current: 0,
        })
    }

    pub fn bottom_window_id(&self) -> Option<winit::window::WindowId> {
        self.bottom.as_ref().map(|(w, _)| w.id())
    }

    fn surface_at(&self, index: usize) -> (&Window, &GlWindowSurface) {
        match (index, &self.bottom) {
            (1, Some((w, s))) => (w, s),
            _ => (&self.window, &self.gl_surface),
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn resize(&self, size: winit::dpi::PhysicalSize<u32>) {
        self.resize_window(self.window.id(), size);
    }

    pub fn resize_window(&self, id: winit::window::WindowId, size: winit::dpi::PhysicalSize<u32>) {
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        let surface = match &self.bottom {
            Some((window, surface)) if window.id() == id => surface,
            _ => &self.gl_surface,
        };
        surface.resize(&self.context, w, h);
    }
}

fn bottom_window(
    events: &ActiveEventLoop,
    top: &Window,
    display: &Display,
    config: &Config,
    context: &PossiblyCurrentContext,
) -> Result<(Window, GlWindowSurface), GfxError> {
    let mut attrs = Window::default_attributes()
        .with_title("slot. bottom screen")
        .with_inner_size(winit::dpi::PhysicalSize::<u32>::from(size()));
    if let Ok(at) = top.outer_position() {
        let below = at.y + top.outer_size().height as i32;
        attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(at.x, below));
    }
    let window = events.create_window(attrs).map_err(err)?;
    let surface_attrs = window
        .build_surface_attributes(Default::default())
        .map_err(err)?;
    let surface = unsafe { display.create_window_surface(config, &surface_attrs) }.map_err(err)?;
    context.make_current(&surface).map_err(err)?;
    let _ = surface.set_swap_interval(context, SwapInterval::DontWait);
    Ok((window, surface))
}

fn pick_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    let mut best: Option<Config> = None;
    for c in configs {
        let better = match &best {
            None => true,
            Some(b) => c.num_samples() < b.num_samples(),
        };
        if better {
            best = Some(c);
        }
    }
    best.unwrap_or_else(|| unreachable!("glutin yielded no configs"))
}

impl Surface for HostSurface {
    fn make_current(&mut self) -> Result<(), GfxError> {
        let (_, surface) = self.surface_at(self.current);
        self.context.make_current(surface).map_err(err)
    }

    fn window_size(&self) -> (u32, u32) {
        let s = self.surface_at(self.current).0.inner_size();
        (s.width, s.height)
    }

    fn swap(&mut self) -> Result<(), GfxError> {
        let (_, surface) = self.surface_at(self.current);
        surface.swap_buffers(&self.context).map_err(err)
    }

    fn panel_count(&self) -> usize {
        1 + usize::from(self.bottom.is_some())
    }

    fn select_panel(&mut self, index: usize) -> Result<(), GfxError> {
        if index >= self.panel_count() {
            return Err(GfxError::Context(format!("no panel {index}")));
        }
        self.current = index;
        self.make_current()
    }

    fn proc_address(&self, name: &str) -> *const c_void {
        match CString::new(name) {
            Ok(c) => self.display.get_proc_address(&c),
            Err(_) => std::ptr::null(),
        }
    }
}
