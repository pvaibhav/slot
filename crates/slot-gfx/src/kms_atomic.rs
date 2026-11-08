use std::ffi::{c_char, c_void, CStr};

use libloading::Library;

use crate::fbdev::{open, sym};
use crate::surface::GfxError;

pub const PLANE_TYPE_PRIMARY: u64 = 1;

const CLIENT_CAP_UNIVERSAL_PLANES: u64 = 2;
const CLIENT_CAP_ATOMIC: u64 = 3;
pub const ATOMIC_NONBLOCK: u32 = 0x0200;
pub const ATOMIC_ALLOW_MODESET: u32 = 0x0400;
pub const PAGE_FLIP_EVENT: u32 = 0x01;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlaneInfo {
    pub id: u32,
    pub possible_crtcs: u32,
    pub kind: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct AtomicIds {
    pub plane: u32,
    pub crtc: u32,
    pub connector: u32,
    pub plane_fb_id: u32,
    pub plane_crtc_id: u32,
    pub plane_src: [u32; 4],
    pub plane_dst: [u32; 4],
    pub crtc_mode_id: u32,
    pub crtc_active: u32,
    pub connector_crtc_id: u32,
}

pub fn primary_plane_for(planes: &[PlaneInfo], crtc_index: usize, taken: &[u32]) -> Option<u32> {
    planes
        .iter()
        .find(|p| {
            p.kind == PLANE_TYPE_PRIMARY
                && p.possible_crtcs & (1 << crtc_index) != 0
                && !taken.contains(&p.id)
        })
        .map(|p| p.id)
}

pub fn atomic_plan(outputs: &[(AtomicIds, u32, (u32, u32), u32)], modeset: bool) -> Vec<(u32, u32, u64)> {
    let mut plan = Vec::new();
    for &(ids, fb, (w, h), mode_blob) in outputs {
        plan.push((ids.plane, ids.plane_fb_id, fb as u64));
        if !modeset {
            continue;
        }
        plan.push((ids.connector, ids.connector_crtc_id, ids.crtc as u64));
        plan.push((ids.crtc, ids.crtc_mode_id, mode_blob as u64));
        plan.push((ids.crtc, ids.crtc_active, 1));
        plan.push((ids.plane, ids.plane_crtc_id, ids.crtc as u64));
        let src = [0, 0, (w as u64) << 16, (h as u64) << 16];
        let dst = [0, 0, w as u64, h as u64];
        for (prop, value) in ids.plane_src.iter().zip(src) {
            plan.push((ids.plane, *prop, value));
        }
        for (prop, value) in ids.plane_dst.iter().zip(dst) {
            plan.push((ids.plane, *prop, value));
        }
    }
    plan
}

#[repr(C)]
struct PlaneRes {
    count_planes: u32,
    planes: *mut u32,
}

#[repr(C)]
struct Plane {
    count_formats: u32,
    formats: *mut u32,
    plane_id: u32,
    crtc_id: u32,
    fb_id: u32,
    crtc_x: u32,
    crtc_y: u32,
    x: u32,
    y: u32,
    possible_crtcs: u32,
    gamma_size: u32,
}

#[repr(C)]
struct ObjectProps {
    count_props: u32,
    props: *mut u32,
    prop_values: *mut u64,
}

#[repr(C)]
struct Property {
    prop_id: u32,
    flags: u32,
    name: [c_char; 32],
}

pub const OBJECT_CRTC: u32 = 0xcccc_cccc;
pub const OBJECT_CONNECTOR: u32 = 0xc0c0_c0c0;
pub const OBJECT_PLANE: u32 = 0xeeee_eeee;

pub struct AtomicApi {
    set_client_cap: unsafe extern "C" fn(i32, u64, u64) -> i32,
    plane_resources: unsafe extern "C" fn(i32) -> *mut PlaneRes,
    free_plane_resources: unsafe extern "C" fn(*mut PlaneRes),
    get_plane: unsafe extern "C" fn(i32, u32) -> *mut Plane,
    free_plane: unsafe extern "C" fn(*mut Plane),
    object_props: unsafe extern "C" fn(i32, u32, u32) -> *mut ObjectProps,
    free_object_props: unsafe extern "C" fn(*mut ObjectProps),
    get_property: unsafe extern "C" fn(i32, u32) -> *mut Property,
    free_property: unsafe extern "C" fn(*mut Property),
    create_blob: unsafe extern "C" fn(i32, *const c_void, usize, *mut u32) -> i32,
    alloc: unsafe extern "C" fn() -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    add_property: unsafe extern "C" fn(*mut c_void, u32, u32, u64) -> i32,
    commit: unsafe extern "C" fn(i32, *mut c_void, u32, *mut c_void) -> i32,
    _lib: Library,
}

impl AtomicApi {
    pub fn load() -> Result<AtomicApi, GfxError> {
        let lib = open("libdrm.so.2").or_else(|_| open("libdrm.so"))?;
        unsafe {
            Ok(AtomicApi {
                set_client_cap: sym(&lib, "drmSetClientCap")?,
                plane_resources: sym(&lib, "drmModeGetPlaneResources")?,
                free_plane_resources: sym(&lib, "drmModeFreePlaneResources")?,
                get_plane: sym(&lib, "drmModeGetPlane")?,
                free_plane: sym(&lib, "drmModeFreePlane")?,
                object_props: sym(&lib, "drmModeObjectGetProperties")?,
                free_object_props: sym(&lib, "drmModeFreeObjectProperties")?,
                get_property: sym(&lib, "drmModeGetProperty")?,
                free_property: sym(&lib, "drmModeFreeProperty")?,
                create_blob: sym(&lib, "drmModeCreatePropertyBlob")?,
                alloc: sym(&lib, "drmModeAtomicAlloc")?,
                free: sym(&lib, "drmModeAtomicFree")?,
                add_property: sym(&lib, "drmModeAtomicAddProperty")?,
                commit: sym(&lib, "drmModeAtomicCommit")?,
                _lib: lib,
            })
        }
    }

    pub fn enable(&self, fd: i32) -> Result<(), GfxError> {
        unsafe {
            if (self.set_client_cap)(fd, CLIENT_CAP_UNIVERSAL_PLANES, 1) != 0
                || (self.set_client_cap)(fd, CLIENT_CAP_ATOMIC, 1) != 0
            {
                return Err(GfxError::Context("the driver refuses atomic modesetting".into()));
            }
        }
        Ok(())
    }

    pub fn planes(&self, fd: i32) -> Vec<PlaneInfo> {
        let mut out = Vec::new();
        unsafe {
            let res = (self.plane_resources)(fd);
            if res.is_null() {
                return out;
            }
            let ids = std::slice::from_raw_parts((*res).planes, (*res).count_planes as usize);
            for &id in ids {
                let p = (self.get_plane)(fd, id);
                if p.is_null() {
                    continue;
                }
                let possible = (*p).possible_crtcs;
                (self.free_plane)(p);
                let kind = self.value(fd, id, OBJECT_PLANE, "type").unwrap_or(u64::MAX);
                out.push(PlaneInfo {
                    id,
                    possible_crtcs: possible,
                    kind,
                });
            }
            (self.free_plane_resources)(res);
        }
        out
    }

    fn find(&self, fd: i32, object: u32, kind: u32, name: &str) -> Option<(u32, u64)> {
        unsafe {
            let props = (self.object_props)(fd, object, kind);
            if props.is_null() {
                return None;
            }
            let n = (*props).count_props as usize;
            let ids = std::slice::from_raw_parts((*props).props, n);
            let values = std::slice::from_raw_parts((*props).prop_values, n);
            let mut found = None;
            for (i, &id) in ids.iter().enumerate() {
                let p = (self.get_property)(fd, id);
                if p.is_null() {
                    continue;
                }
                let matches = CStr::from_ptr((*p).name.as_ptr()).to_bytes() == name.as_bytes();
                (self.free_property)(p);
                if matches {
                    found = Some((id, values[i]));
                    break;
                }
            }
            (self.free_object_props)(props);
            found
        }
    }

    pub fn prop(&self, fd: i32, object: u32, kind: u32, name: &str) -> Result<u32, GfxError> {
        self.find(fd, object, kind, name)
            .map(|(id, _)| id)
            .ok_or_else(|| GfxError::Context(format!("drm object {object} has no {name} property")))
    }

    fn value(&self, fd: i32, object: u32, kind: u32, name: &str) -> Option<u64> {
        self.find(fd, object, kind, name).map(|(_, v)| v)
    }

    pub fn ids(&self, fd: i32, plane: u32, crtc: u32, connector: u32) -> Result<AtomicIds, GfxError> {
        let p = |name| self.prop(fd, plane, OBJECT_PLANE, name);
        Ok(AtomicIds {
            plane,
            crtc,
            connector,
            plane_fb_id: p("FB_ID")?,
            plane_crtc_id: p("CRTC_ID")?,
            plane_src: [p("SRC_X")?, p("SRC_Y")?, p("SRC_W")?, p("SRC_H")?],
            plane_dst: [p("CRTC_X")?, p("CRTC_Y")?, p("CRTC_W")?, p("CRTC_H")?],
            crtc_mode_id: self.prop(fd, crtc, OBJECT_CRTC, "MODE_ID")?,
            crtc_active: self.prop(fd, crtc, OBJECT_CRTC, "ACTIVE")?,
            connector_crtc_id: self.prop(fd, connector, OBJECT_CONNECTOR, "CRTC_ID")?,
        })
    }

    pub fn mode_blob<T>(&self, fd: i32, mode: &T) -> Result<u32, GfxError> {
        let mut id = 0;
        let rc = unsafe {
            (self.create_blob)(fd, mode as *const T as *const c_void, std::mem::size_of::<T>(), &mut id)
        };
        match rc {
            0 => Ok(id),
            e => Err(GfxError::Context(format!("drmModeCreatePropertyBlob: {e}"))),
        }
    }

    pub fn commit(&self, fd: i32, plan: &[(u32, u32, u64)], flags: u32, data: *mut c_void) -> Result<(), GfxError> {
        unsafe {
            let req = (self.alloc)();
            if req.is_null() {
                return Err(GfxError::Context("drmModeAtomicAlloc failed".into()));
            }
            for &(object, prop, value) in plan {
                if (self.add_property)(req, object, prop, value) < 0 {
                    (self.free)(req);
                    return Err(GfxError::Context(format!("drmModeAtomicAddProperty {object}.{prop}")));
                }
            }
            let rc = (self.commit)(fd, req, flags, data);
            (self.free)(req);
            match rc {
                0 => Ok(()),
                e => Err(GfxError::Context(format!("drmModeAtomicCommit: {e}"))),
            }
        }
    }
}
