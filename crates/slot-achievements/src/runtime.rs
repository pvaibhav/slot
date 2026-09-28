use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr::NonNull;

unsafe extern "C" {
    fn rc_runtime_alloc() -> *mut c_void;
    fn rc_runtime_destroy(runtime: *mut c_void);
    fn rc_runtime_activate_achievement(
        runtime: *mut c_void,
        id: u32,
        definition: *const c_char,
        lua: *mut c_void,
        funcs: c_int,
    ) -> c_int;
    fn rc_runtime_deactivate_achievement(runtime: *mut c_void, id: u32);
    fn rc_runtime_reset(runtime: *mut c_void);
    fn rc_runtime_activate_richpresence(
        runtime: *mut c_void,
        script: *const c_char,
        lua: *mut c_void,
        funcs: c_int,
    ) -> c_int;
    fn slot_ra_frame(
        runtime: *mut c_void,
        ram: *const u8,
        valid: *const usize,
        earned: *mut u32,
        capacity: usize,
        presence: *mut c_char,
        presence_size: usize,
    ) -> usize;
}

pub(crate) struct Runtime {
    ptr: NonNull<c_void>,
    earned: Vec<u32>,
    presence: [u8; 1024],
    script: String,
}

impl Runtime {
    pub fn new() -> Option<Self> {
        Some(Self {
            ptr: NonNull::new(unsafe { rc_runtime_alloc() })?,
            earned: Vec::new(),
            presence: [0; 1024],
            script: String::new(),
        })
    }

    pub fn activate(&mut self, id: u32, definition: &str) -> bool {
        let Ok(definition) = CString::new(definition) else {
            return false;
        };
        let result = unsafe {
            rc_runtime_activate_achievement(
                self.ptr.as_ptr(),
                id,
                definition.as_ptr(),
                std::ptr::null_mut(),
                0,
            )
        };
        if result == 0 {
            self.earned.push(0);
        }
        result == 0
    }

    pub fn activate_presence(&mut self, script: &str) -> bool {
        if !script.is_empty() && script == self.script {
            return true;
        }
        self.presence.fill(0);
        let activate = |script: &str| {
            let Ok(script) = CString::new(script) else {
                return false;
            };
            unsafe {
                rc_runtime_activate_richpresence(
                    self.ptr.as_ptr(),
                    script.as_ptr(),
                    std::ptr::null_mut(),
                    0,
                ) == 0
            }
        };
        if activate(script) {
            self.script = script.into();
            true
        } else {
            // A failed replacement must not continue reporting the previous script.
            activate("Display:\n ");
            self.script.clear();
            false
        }
    }

    pub fn presence(&self) -> String {
        let len = self
            .presence
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(self.presence.len());
        String::from_utf8_lossy(&self.presence[..len])
            .trim()
            .to_owned()
    }

    pub fn deactivate(&mut self, id: u32) {
        unsafe {
            rc_runtime_deactivate_achievement(self.ptr.as_ptr(), id);
        }
    }

    pub fn reset(&mut self) {
        unsafe {
            rc_runtime_reset(self.ptr.as_ptr());
        }
    }

    pub fn frame(&mut self, ram: &[u8], valid: &[usize; 3]) -> &[u32] {
        assert!(ram.len() >= 0x58000);
        assert!(valid[0] <= 0x8000 && valid[1] <= 0x40000 && valid[2] <= 0x10000);
        let count = unsafe {
            slot_ra_frame(
                self.ptr.as_ptr(),
                ram.as_ptr(),
                valid.as_ptr(),
                self.earned.as_mut_ptr(),
                self.earned.len(),
                self.presence.as_mut_ptr().cast(),
                self.presence.len(),
            )
        };
        &self.earned[..count]
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            rc_runtime_destroy(self.ptr.as_ptr());
        }
    }
}
