use std::ffi::{c_char, c_void};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use libloading::Library;

use crate::fbdev::{
    gl_proc, open, sym, Egl, Ptr, EGL_ALPHA_SIZE, EGL_BLUE_SIZE, EGL_CONTEXT_CLIENT_VERSION,
    EGL_GREEN_SIZE, EGL_NONE, EGL_OPENGL_ES2_BIT, EGL_OPENGL_ES_API, EGL_RED_SIZE,
    EGL_RENDERABLE_TYPE, EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
};
use crate::surface::{GfxError, Surface};

const CARD: &str = "/dev/dri/card0";
const ATOMIC_FLAG: &str = "/mnt/sdcard/System/kms-atomic.on";

pub const CONNECTOR_DSI: u32 = 16;
const CONNECTED: i32 = 1;
pub const MODE_TYPE_PREFERRED: u32 = 1 << 3;
const PAGE_FLIP_EVENT: u32 = 1;

const GBM_FORMAT_XRGB8888: u32 = 0x3432_5258;
const GBM_BO_USE_SCANOUT: u32 = 1;
const GBM_BO_USE_RENDERING: u32 = 1 << 2;

const EGL_NATIVE_VISUAL_ID: i32 = 0x302E;
const EGL_PLATFORM_GBM_KHR: u32 = 0x31D7;

const POLLIN: i16 = 1;

const MASTER_TRIES: u32 = 500;
const MASTER_WAIT: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Top,
    Bottom,
}

impl Panel {
    pub fn connector(self) -> (u32, u32) {
        match self {
            Panel::Top => (CONNECTOR_DSI, 2),
            Panel::Bottom => (CONNECTOR_DSI, 1),
        }
    }

    fn index(self) -> usize {
        match self {
            Panel::Top => 0,
            Panel::Bottom => 1,
        }
    }

    pub fn of_index(index: usize) -> Option<Panel> {
        match index {
            0 => Some(Panel::Top),
            1 => Some(Panel::Bottom),
            _ => None,
        }
    }
}

#[repr(C)]
struct DrmRes {
    count_fbs: i32,
    fbs: *mut u32,
    count_crtcs: i32,
    crtcs: *mut u32,
    count_connectors: i32,
    connectors: *mut u32,
    count_encoders: i32,
    encoders: *mut u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ModeInfo {
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,
    vrefresh: u32,
    flags: u32,
    kind: u32,
    name: [c_char; 32],
}

#[repr(C)]
struct DrmConnector {
    connector_id: u32,
    encoder_id: u32,
    connector_type: u32,
    connector_type_id: u32,
    connection: i32,
    mm_width: u32,
    mm_height: u32,
    subpixel: i32,
    count_modes: i32,
    modes: *mut ModeInfo,
    count_props: i32,
    props: *mut u32,
    prop_values: *mut u64,
    count_encoders: i32,
    encoders: *mut u32,
}

#[repr(C)]
struct DrmEncoder {
    encoder_id: u32,
    encoder_type: u32,
    crtc_id: u32,
    possible_crtcs: u32,
    possible_clones: u32,
}

type FlipHandler = unsafe extern "C" fn(i32, u32, u32, u32, *mut c_void);

#[repr(C)]
struct EventContext {
    version: i32,
    vblank_handler: Option<FlipHandler>,
    page_flip_handler: Option<FlipHandler>,
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

const CLOCK_MONOTONIC: i32 = 1;

extern "C" {
    fn poll(fds: *mut PollFd, count: u64, timeout: i32) -> i32;
    fn clock_gettime(clock: i32, ts: *mut Timespec) -> i32;
}

struct Drm {
    get_resources: unsafe extern "C" fn(i32) -> *mut DrmRes,
    free_resources: unsafe extern "C" fn(*mut DrmRes),
    get_connector: unsafe extern "C" fn(i32, u32) -> *mut DrmConnector,
    free_connector: unsafe extern "C" fn(*mut DrmConnector),
    get_encoder: unsafe extern "C" fn(i32, u32) -> *mut DrmEncoder,
    free_encoder: unsafe extern "C" fn(*mut DrmEncoder),
    add_fb: unsafe extern "C" fn(i32, u32, u32, u8, u8, u32, u32, *mut u32) -> i32,
    rm_fb: unsafe extern "C" fn(i32, u32) -> i32,
    set_crtc:
        unsafe extern "C" fn(i32, u32, u32, u32, u32, *const u32, i32, *const ModeInfo) -> i32,
    page_flip: unsafe extern "C" fn(i32, u32, u32, u32, *mut c_void) -> i32,
    handle_event: unsafe extern "C" fn(i32, *mut EventContext) -> i32,
    set_master: unsafe extern "C" fn(i32) -> i32,
    _lib: Library,
}

impl Drm {
    fn load() -> Result<Self, GfxError> {
        let lib = open("libdrm.so.2").or_else(|_| open("libdrm.so"))?;
        unsafe {
            Ok(Drm {
                get_resources: sym(&lib, "drmModeGetResources")?,
                free_resources: sym(&lib, "drmModeFreeResources")?,
                get_connector: sym(&lib, "drmModeGetConnector")?,
                free_connector: sym(&lib, "drmModeFreeConnector")?,
                get_encoder: sym(&lib, "drmModeGetEncoder")?,
                free_encoder: sym(&lib, "drmModeFreeEncoder")?,
                add_fb: sym(&lib, "drmModeAddFB")?,
                rm_fb: sym(&lib, "drmModeRmFB")?,
                set_crtc: sym(&lib, "drmModeSetCrtc")?,
                page_flip: sym(&lib, "drmModePageFlip")?,
                handle_event: sym(&lib, "drmHandleEvent")?,
                set_master: sym(&lib, "drmSetMaster")?,
                _lib: lib,
            })
        }
    }
}

struct Gbm {
    create_device: unsafe extern "C" fn(i32) -> Ptr,
    device_destroy: unsafe extern "C" fn(Ptr),
    surface_create: unsafe extern "C" fn(Ptr, u32, u32, u32, u32) -> Ptr,
    surface_destroy: unsafe extern "C" fn(Ptr),
    lock_front_buffer: unsafe extern "C" fn(Ptr) -> Ptr,
    release_buffer: unsafe extern "C" fn(Ptr, Ptr),
    has_free_buffers: unsafe extern "C" fn(Ptr) -> i32,
    bo_get_handle: unsafe extern "C" fn(Ptr) -> u64,
    bo_get_stride: unsafe extern "C" fn(Ptr) -> u32,
    _lib: Library,
}

impl Gbm {
    fn load() -> Result<Self, GfxError> {
        let lib = open("libgbm.so.1").or_else(|_| open("libgbm.so"))?;
        unsafe {
            Ok(Gbm {
                create_device: sym(&lib, "gbm_create_device")?,
                device_destroy: sym(&lib, "gbm_device_destroy")?,
                surface_create: sym(&lib, "gbm_surface_create")?,
                surface_destroy: sym(&lib, "gbm_surface_destroy")?,
                lock_front_buffer: sym(&lib, "gbm_surface_lock_front_buffer")?,
                release_buffer: sym(&lib, "gbm_surface_release_buffer")?,
                has_free_buffers: sym(&lib, "gbm_surface_has_free_buffers")?,
                bo_get_handle: sym(&lib, "gbm_bo_get_handle")?,
                bo_get_stride: sym(&lib, "gbm_bo_get_stride")?,
                _lib: lib,
            })
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FlipStep<T> {
    pub flip: Option<T>,
    pub release: Option<T>,
}

#[derive(Debug)]
pub struct FlipQueue<T> {
    shown: Option<T>,
    pending: Option<T>,
    waiting: Option<T>,
}

impl<T> Default for FlipQueue<T> {
    fn default() -> Self {
        FlipQueue {
            shown: None,
            pending: None,
            waiting: None,
        }
    }
}

impl<T: Copy> FlipQueue<T> {
    pub fn show(&mut self, frame: T) -> Option<T> {
        self.shown.replace(frame)
    }

    pub fn submit(&mut self, frame: T) -> FlipStep<T> {
        match self.pending {
            None => {
                self.pending = Some(frame);
                FlipStep { flip: Some(frame), release: None }
            }
            Some(_) => FlipStep {
                flip: None,
                release: self.waiting.replace(frame),
            },
        }
    }

    pub fn retire(&mut self) -> FlipStep<T> {
        let Some(done) = self.pending.take() else {
            return FlipStep { flip: None, release: None };
        };
        let release = self.shown.replace(done);
        self.pending = self.waiting.take();
        FlipStep { flip: self.pending, release }
    }

    pub fn flip_failed(&mut self) -> Option<T> {
        self.pending.take()
    }

    pub fn in_flight(&self) -> bool {
        self.pending.is_some()
    }
}

struct Output {
    connector: u32,
    crtc: u32,
    mode: ModeInfo,
    size: (u32, u32),
    gbm_surface: Ptr,
    egl_surface: Ptr,
    shown: Ptr,
    queued: Ptr,
    flipping: Box<Flip>,
    flips: FlipQueue<(Ptr, u32)>,
    lit: bool,
    fbs: Vec<(Ptr, u32)>,
}

pub struct KmsSurface {
    egl: Egl,
    gles: Library,
    drm: Drm,
    gbm: Gbm,
    display: Ptr,
    context: Ptr,
    device: Ptr,
    outputs: [Output; 2],
    current: usize,
    card: File,
    master: bool,
    atomic: Option<Atomic>,
}

pub fn retry_until(mut f: impl FnMut() -> bool, tries: u32, wait: Duration) -> bool {
    for i in 0..tries {
        if f() {
            return true;
        }
        if i + 1 < tries {
            std::thread::sleep(wait);
        }
    }
    false
}

pub fn preferred_mode(kinds: &[u32]) -> Option<usize> {
    kinds
        .iter()
        .position(|k| k & MODE_TYPE_PREFERRED != 0)
        .or((!kinds.is_empty()).then_some(0))
}

pub fn free_crtc(current: u32, possible: u32, crtcs: &[u32], taken: &[u32]) -> Option<u32> {
    if current != 0 && !taken.contains(&current) {
        return Some(current);
    }
    crtcs
        .iter()
        .enumerate()
        .find(|(i, c)| possible & (1 << i) != 0 && !taken.contains(c))
        .map(|(_, c)| *c)
}

unsafe extern "C" fn flipped_one(_fd: i32, _seq: u32, _sec: u32, _usec: u32, data: *mut c_void) {
    let left = &*(data as *const AtomicU32);
    let _ = left.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
}

struct Atomic {
    api: crate::kms_atomic::AtomicApi,
    ids: [crate::kms_atomic::AtomicIds; 2],
    blobs: [u32; 2],
    pending: [Option<(Ptr, u32)>; 2],
    left: Box<AtomicU32>,
}

unsafe fn atomic_for(fd: i32, found: &[Found; 2]) -> Result<Atomic, GfxError> {
    let api = crate::kms_atomic::AtomicApi::load()?;
    api.enable(fd)?;
    let planes = api.planes(fd);
    let mut taken = Vec::new();
    let mut ids = [crate::kms_atomic::AtomicIds::default(); 2];
    let mut blobs = [0u32; 2];
    for (i, f) in found.iter().enumerate() {
        let plane = crate::kms_atomic::primary_plane_for(&planes, f.crtc_index, &taken)
            .ok_or_else(|| GfxError::Context(format!("no primary plane for crtc {}", f.crtc)))?;
        taken.push(plane);
        ids[i] = api.ids(fd, plane, f.crtc, f.connector)?;
        blobs[i] = api.mode_blob(fd, &f.mode)?;
    }
    Ok(Atomic {
        api,
        ids,
        blobs,
        pending: [None, None],
        left: Box::new(AtomicU32::new(0)),
    })
}

#[derive(Default)]
struct Flip {
    busy: AtomicBool,
    at_us: std::sync::atomic::AtomicU64,
}

unsafe extern "C" fn flipped(_fd: i32, _seq: u32, sec: u32, usec: u32, data: *mut c_void) {
    let flip = &*(data as *const Flip);
    flip.at_us.store(u64::from(sec) * 1_000_000 + u64::from(usec), Ordering::Release);
    flip.busy.store(false, Ordering::Release);
}

struct Found {
    connector: u32,
    crtc: u32,
    crtc_index: usize,
    mode: ModeInfo,
}

unsafe fn find(drm: &Drm, fd: i32) -> Result<[Found; 2], GfxError> {
    let res = (drm.get_resources)(fd);
    if res.is_null() {
        return Err(GfxError::Context(format!("{CARD}: no drm resources")));
    }
    let r = &*res;
    let connectors = std::slice::from_raw_parts(r.connectors, r.count_connectors.max(0) as usize);
    let crtcs = std::slice::from_raw_parts(r.crtcs, r.count_crtcs.max(0) as usize);
    let mut found: [Option<Found>; 2] = [None, None];
    let mut taken = Vec::new();
    for panel in [Panel::Top, Panel::Bottom] {
        let (kind, id) = panel.connector();
        for &cid in connectors {
            let c = (drm.get_connector)(fd, cid);
            if c.is_null() {
                continue;
            }
            let conn = &*c;
            let wanted = conn.connector_type == kind
                && conn.connector_type_id == id
                && conn.connection == CONNECTED
                && conn.count_modes > 0;
            if wanted {
                let modes = std::slice::from_raw_parts(conn.modes, conn.count_modes as usize);
                let kinds: Vec<u32> = modes.iter().map(|m| m.kind).collect();
                let mode = modes[preferred_mode(&kinds).unwrap_or(0)];
                let mut crtc = None;
                let encoders =
                    std::slice::from_raw_parts(conn.encoders, conn.count_encoders.max(0) as usize);
                for &eid in encoders {
                    let e = (drm.get_encoder)(fd, eid);
                    if e.is_null() {
                        continue;
                    }
                    let current = if eid == conn.encoder_id {
                        (*e).crtc_id
                    } else {
                        0
                    };
                    crtc = free_crtc(current, (*e).possible_crtcs, crtcs, &taken);
                    (drm.free_encoder)(e);
                    if crtc.is_some() {
                        break;
                    }
                }
                if let Some(crtc) = crtc {
                    taken.push(crtc);
                    found[panel.index()] = Some(Found {
                        connector: cid,
                        crtc,
                        crtc_index: crtcs.iter().position(|&c| c == crtc).unwrap_or(0),
                        mode,
                    });
                }
            }
            (drm.free_connector)(c);
        }
    }
    (drm.free_resources)(res);
    match found {
        [Some(top), Some(bottom)] => Ok([top, bottom]),
        [top, _] => Err(GfxError::Context(format!(
            "{CARD}: no connected DSI-{} with a free crtc",
            if top.is_none() { 2 } else { 1 }
        ))),
    }
}

unsafe fn choose_config(egl: &Egl, display: Ptr) -> Result<Ptr, GfxError> {
    let attrs = [
        EGL_SURFACE_TYPE,
        EGL_WINDOW_BIT,
        EGL_RED_SIZE,
        8,
        EGL_GREEN_SIZE,
        8,
        EGL_BLUE_SIZE,
        8,
        EGL_ALPHA_SIZE,
        0,
        EGL_RENDERABLE_TYPE,
        EGL_OPENGL_ES2_BIT,
        EGL_NONE,
    ];
    let mut configs = [std::ptr::null_mut(); 64];
    let mut count = 0;
    if (egl.choose_config)(
        display,
        attrs.as_ptr(),
        configs.as_mut_ptr(),
        64,
        &mut count,
    ) == 0
    {
        return Err(egl.fail("eglChooseConfig"));
    }
    configs[..count.max(0) as usize]
        .iter()
        .copied()
        .find(|&c| {
            let mut id = 0;
            (egl.get_config_attrib)(display, c, EGL_NATIVE_VISUAL_ID, &mut id) != 0
                && id as u32 == GBM_FORMAT_XRGB8888
        })
        .ok_or_else(|| GfxError::Context("eglChooseConfig: no XRGB8888 es2 config".into()))
}

impl KmsSurface {
    pub fn new() -> Result<Self, GfxError> {
        let card = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(CARD)
            .map_err(|e| GfxError::Context(format!("{CARD}: {e}")))?;
        let fd = card.as_raw_fd();
        let drm = Drm::load()?;
        let gbm = Gbm::load()?;
        let egl = Egl::load()?;
        let gles = open("libGLESv2.so.2").or_else(|_| open("libGLESv2.so"))?;
        unsafe {
            let found = find(&drm, fd)?;
            let atomic = match std::path::Path::new(ATOMIC_FLAG).is_file() {
                true => atomic_for(fd, &found),
                false => Err(GfxError::Context(format!("{ATOMIC_FLAG} is not set"))),
            };
            let atomic = match atomic {
                Ok(a) => {
                    eprintln!("slot: kms: both panels flip in one atomic commit");
                    Some(a)
                }
                Err(e) => {
                    eprintln!("slot: kms: atomic unavailable ({e}), flipping each panel on its own");
                    None
                }
            };
            let device = (gbm.create_device)(fd);
            if device.is_null() {
                return Err(GfxError::Context("gbm_create_device failed".into()));
            }
            type PlatformDisplay = unsafe extern "C" fn(u32, Ptr, *const isize) -> Ptr;
            let display = match egl.lib.get::<PlatformDisplay>(b"eglGetPlatformDisplay") {
                Ok(f) => f(EGL_PLATFORM_GBM_KHR, device, std::ptr::null()),
                Err(_) => (egl.get_display)(device),
            };
            if display.is_null() {
                return Err(egl.fail("eglGetPlatformDisplay"));
            }
            if (egl.initialize)(display, std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                return Err(egl.fail("eglInitialize"));
            }
            if (egl.bind_api)(EGL_OPENGL_ES_API) == 0 {
                return Err(egl.fail("eglBindAPI"));
            }
            let config = choose_config(&egl, display)?;
            let context_attrs = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
            let context = (egl.create_context)(
                display,
                config,
                std::ptr::null_mut(),
                context_attrs.as_ptr(),
            );
            if context.is_null() {
                return Err(egl.fail("eglCreateContext"));
            }
            let mut outputs = Vec::with_capacity(2);
            for f in found {
                let size = (f.mode.hdisplay as u32, f.mode.vdisplay as u32);
                let gbm_surface = (gbm.surface_create)(
                    device,
                    size.0,
                    size.1,
                    GBM_FORMAT_XRGB8888,
                    GBM_BO_USE_SCANOUT | GBM_BO_USE_RENDERING,
                );
                if gbm_surface.is_null() {
                    return Err(GfxError::Context("gbm_surface_create failed".into()));
                }
                let egl_surface =
                    (egl.create_window_surface)(display, config, gbm_surface, std::ptr::null());
                if egl_surface.is_null() {
                    return Err(egl.fail("eglCreateWindowSurface"));
                }
                eprintln!(
                    "slot: kms connector {} crtc {} {}x{}@{}",
                    f.connector, f.crtc, size.0, size.1, f.mode.vrefresh
                );
                outputs.push(Output {
                    connector: f.connector,
                    crtc: f.crtc,
                    mode: f.mode,
                    size,
                    gbm_surface,
                    egl_surface,
                    shown: std::ptr::null_mut(),
                    queued: std::ptr::null_mut(),
                    flipping: Box::new(Flip::default()),
                    flips: FlipQueue::default(),
                    lit: false,
                    fbs: Vec::new(),
                });
            }
            let bottom = outputs.pop().expect("two outputs");
            let top = outputs.pop().expect("two outputs");
            let mut surface = KmsSurface {
                egl,
                gles,
                drm,
                gbm,
                display,
                context,
                device,
                outputs: [top, bottom],
                current: 0,
                card,
                master: false,
                atomic,
            };
            surface.select(Panel::Top)?;
            Ok(surface)
        }
    }

    pub fn select(&mut self, panel: Panel) -> Result<(), GfxError> {
        self.current = panel.index();
        self.make_current()
    }

    pub fn panel_size(&self, panel: Panel) -> (u32, u32) {
        self.outputs[panel.index()].size
    }

    fn take_master(&mut self) {
        if self.master {
            return;
        }
        let fd = self.card.as_raw_fd();
        let set_master = self.drm.set_master;
        if !retry_until(|| unsafe { set_master(fd) } == 0, MASTER_TRIES, MASTER_WAIT) {
            eprintln!("slot: kms: the display is still held by another program");
        }
        self.master = true;
    }

    fn fb_for(&mut self, bo: Ptr) -> Result<u32, GfxError> {
        let out = &mut self.outputs[self.current];
        if let Some(&(_, fb)) = out.fbs.iter().find(|(b, _)| *b == bo) {
            return Ok(fb);
        }
        let mut fb = 0;
        let rc = unsafe {
            let handle = (self.gbm.bo_get_handle)(bo) as u32;
            let stride = (self.gbm.bo_get_stride)(bo);
            (self.drm.add_fb)(
                self.card.as_raw_fd(),
                out.size.0,
                out.size.1,
                24,
                32,
                stride,
                handle,
                &mut fb,
            )
        };
        if rc != 0 {
            return Err(GfxError::Context(format!("drmModeAddFB: {rc}")));
        }
        out.fbs.push((bo, fb));
        Ok(fb)
    }

    fn wait_atomic(&mut self) -> Result<(), GfxError> {
        let fd = self.card.as_raw_fd();
        let Some(atomic) = self.atomic.as_ref() else {
            return Ok(());
        };
        while atomic.left.load(Ordering::Acquire) > 0 {
            let mut pfd = PollFd {
                fd,
                events: POLLIN,
                revents: 0,
            };
            if unsafe { poll(&mut pfd, 1, -1) } < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(GfxError::Context(format!("poll {CARD}: {e}")));
            }
            let mut ctx = EventContext {
                version: 2,
                vblank_handler: None,
                page_flip_handler: Some(flipped_one),
            };
            unsafe { (self.drm.handle_event)(fd, &mut ctx) };
        }
        for out in self.outputs.iter_mut() {
            if !out.queued.is_null() {
                if !out.shown.is_null() {
                    unsafe { (self.gbm.release_buffer)(out.gbm_surface, out.shown) };
                }
                out.shown = std::mem::replace(&mut out.queued, std::ptr::null_mut());
            }
        }
        Ok(())
    }

    fn commit_atomic(&mut self) -> Result<(), GfxError> {
        self.take_master();
        self.wait_atomic()?;
        let fd = self.card.as_raw_fd();
        let Some(atomic) = self.atomic.as_mut() else {
            return Ok(());
        };
        let mut plan = Vec::new();
        let mut modeset = false;
        let mut taking = Vec::new();
        for (i, out) in self.outputs.iter().enumerate() {
            let Some((bo, fb)) = atomic.pending[i].take() else {
                continue;
            };
            let first = !out.lit;
            modeset |= first;
            plan.extend(crate::kms_atomic::atomic_plan(
                &[(atomic.ids[i], fb, out.size, atomic.blobs[i])],
                first,
            ));
            taking.push((i, bo));
        }
        if taking.is_empty() {
            return Ok(());
        }
        let left = &*atomic.left as *const AtomicU32 as *mut c_void;
        let flags = match modeset {
            true => crate::kms_atomic::ATOMIC_ALLOW_MODESET,
            false => crate::kms_atomic::ATOMIC_NONBLOCK | crate::kms_atomic::PAGE_FLIP_EVENT,
        };
        if !modeset {
            atomic.left.store(taking.len() as u32, Ordering::Release);
        }
        if let Err(e) = atomic.api.commit(fd, &plan, flags, left) {
            atomic.left.store(0, Ordering::Release);
            for (i, bo) in taking {
                unsafe { (self.gbm.release_buffer)(self.outputs[i].gbm_surface, bo) };
            }
            return Err(e);
        }
        for (i, bo) in taking {
            let out = &mut self.outputs[i];
            match modeset {
                true => {
                    if !out.shown.is_null() {
                        unsafe { (self.gbm.release_buffer)(out.gbm_surface, out.shown) };
                    }
                    out.shown = bo;
                    out.lit = true;
                }
                false => out.queued = bo,
            }
        }
        Ok(())
    }

    fn swap_atomic(&mut self) -> Result<(), GfxError> {
        let s = self.outputs[self.current].egl_surface;
        if unsafe { (self.egl.swap_buffers)(self.display, s) } == 0 {
            return Err(self.egl.fail("eglSwapBuffers"));
        }
        let already = self
            .atomic
            .as_ref()
            .is_some_and(|a| a.pending[self.current].is_some());
        if already {
            self.commit_atomic()?;
        }
        let gbm_surface = self.outputs[self.current].gbm_surface;
        let bo = unsafe { (self.gbm.lock_front_buffer)(gbm_surface) };
        if bo.is_null() {
            return Err(GfxError::Context(
                "gbm_surface_lock_front_buffer failed".into(),
            ));
        }
        let fb = self.fb_for(bo)?;
        let current = self.current;
        let full = match self.atomic.as_mut() {
            Some(a) => {
                a.pending[current] = Some((bo, fb));
                a.pending.iter().all(Option::is_some)
            }
            None => false,
        };
        if full {
            self.commit_atomic()?;
        }
        Ok(())
    }

    fn pump(&mut self, block: bool) -> Result<(), GfxError> {
        let fd = self.card.as_raw_fd();
        let mut pfd = PollFd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        let ready = unsafe { poll(&mut pfd, 1, if block { 1000 } else { 0 }) };
        if ready < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(GfxError::Context(format!("poll {CARD}: {e}")));
        }
        if ready == 0 {
            return match block {
                true => Err(GfxError::Context(format!("{CARD}: no flip completed in 1 s"))),
                false => Ok(()),
            };
        }
        let mut ctx = EventContext {
            version: 2,
            vblank_handler: None,
            page_flip_handler: Some(flipped),
        };
        unsafe { (self.drm.handle_event)(fd, &mut ctx) };
        for i in 0..self.outputs.len() {
            let out = &self.outputs[i];
            if out.flips.in_flight() && !out.flipping.busy.load(Ordering::Acquire) {
                let step = self.outputs[i].flips.retire();
                self.apply(i, step)?;
            }
        }
        Ok(())
    }

    fn apply(&mut self, i: usize, step: FlipStep<(Ptr, u32)>) -> Result<(), GfxError> {
        let fd = self.card.as_raw_fd();
        let out = &mut self.outputs[i];
        if let Some((bo, _)) = step.release {
            unsafe { (self.gbm.release_buffer)(out.gbm_surface, bo) };
        }
        let Some((_, fb)) = step.flip else {
            return Ok(());
        };
        out.flipping.busy.store(true, Ordering::Release);
        let data = &*out.flipping as *const Flip as *mut c_void;
        let rc = unsafe { (self.drm.page_flip)(fd, out.crtc, fb, PAGE_FLIP_EVENT, data) };
        if rc != 0 {
            out.flipping.busy.store(false, Ordering::Release);
            if let Some((bo, _)) = out.flips.flip_failed() {
                unsafe { (self.gbm.release_buffer)(out.gbm_surface, bo) };
            }
            return Err(GfxError::Context(format!("drmModePageFlip: {rc}")));
        }
        Ok(())
    }

    fn settle(&mut self) {
        while self.outputs.iter().any(|o| o.flips.in_flight()) {
            if self.pump(true).is_err() {
                break;
            }
        }
    }
}

impl Surface for KmsSurface {
    fn make_current(&mut self) -> Result<(), GfxError> {
        let s = self.outputs[self.current].egl_surface;
        match unsafe { (self.egl.make_current)(self.display, s, s, self.context) } {
            0 => Err(self.egl.fail("eglMakeCurrent")),
            _ => Ok(()),
        }
    }

    fn window_size(&self) -> (u32, u32) {
        self.outputs[self.current].size
    }

    fn swap(&mut self) -> Result<(), GfxError> {
        if self.atomic.is_some() {
            return self.swap_atomic();
        }
        let s = self.outputs[self.current].egl_surface;
        if unsafe { (self.egl.swap_buffers)(self.display, s) } == 0 {
            return Err(self.egl.fail("eglSwapBuffers"));
        }
        self.pump(false)?;
        let gbm_surface = self.outputs[self.current].gbm_surface;
        let bo = unsafe { (self.gbm.lock_front_buffer)(gbm_surface) };
        if bo.is_null() {
            return Err(GfxError::Context(
                "gbm_surface_lock_front_buffer failed".into(),
            ));
        }
        let fb = self.fb_for(bo)?;
        let fd = self.card.as_raw_fd();
        let i = self.current;
        if !self.outputs[i].lit {
            self.take_master();
            let out = &mut self.outputs[i];
            let rc = unsafe {
                (self.drm.set_crtc)(fd, out.crtc, fb, 0, 0, &out.connector, 1, &out.mode)
            };
            if rc != 0 {
                return Err(GfxError::Context(format!("drmModeSetCrtc: {rc}")));
            }
            out.lit = true;
            if let Some((old, _)) = out.flips.show((bo, fb)) {
                unsafe { (self.gbm.release_buffer)(out.gbm_surface, old) };
            }
            return Ok(());
        }
        let step = self.outputs[i].flips.submit((bo, fb));
        self.apply(i, step)?;
        while unsafe { (self.gbm.has_free_buffers)(gbm_surface) } == 0 {
            self.pump(true)?;
        }
        Ok(())
    }

    fn proc_address(&self, name: &str) -> *const c_void {
        gl_proc(&self.egl, &self.gles, name)
    }

    fn panel_count(&self) -> usize {
        self.outputs.len()
    }

    fn refresh_period(&self) -> Option<std::time::Duration> {
        let m = &self.outputs[Panel::Top.index()].mode;
        crate::surface::mode_period(m.clock, m.htotal, m.vtotal)
    }

    fn flip_times_us(&self) -> Option<[u64; 2]> {
        let at = |i: usize| self.outputs.get(i).map(|o| o.flipping.at_us.load(Ordering::Acquire));
        Some([at(0)?, at(1)?]).filter(|t| t[0] > 0 && t[1] > 0)
    }

    fn last_vblank(&self) -> Option<std::time::Instant> {
        let at = self.outputs[Panel::Top.index()].flipping.at_us.load(Ordering::Acquire);
        let mut ts = Timespec { sec: 0, nsec: 0 };
        if at == 0 || unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) } != 0 {
            return None;
        }
        let now = Instant::now();
        let mono = ts.sec as u64 * 1_000_000 + ts.nsec as u64 / 1000;
        now.checked_sub(Duration::from_micros(mono.saturating_sub(at)))
    }

    fn select_panel(&mut self, index: usize) -> Result<(), GfxError> {
        match Panel::of_index(index) {
            Some(panel) => self.select(panel),
            None => Err(GfxError::Context(format!("no panel {index}"))),
        }
    }
}

impl Drop for KmsSurface {
    fn drop(&mut self) {
        let _ = self.wait_atomic();
        self.settle();
        let fd = self.card.as_raw_fd();
        unsafe {
            (self.egl.make_current)(
                self.display,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (self.egl.terminate)(self.display);
            for out in &self.outputs {
                for &(_, fb) in &out.fbs {
                    (self.drm.rm_fb)(fd, fb);
                }
                (self.gbm.surface_destroy)(out.gbm_surface);
            }
            (self.gbm.device_destroy)(self.device);
        }
    }
}
