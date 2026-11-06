use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct Jack {
    plugged: AtomicBool,
    changed: AtomicBool,
}

impl Jack {
    pub fn set(&self, plugged: bool) {
        if self.plugged.swap(plugged, Ordering::Relaxed) != plugged {
            self.changed.store(true, Ordering::Relaxed);
        }
    }

    pub fn plugged(&self) -> bool {
        self.plugged.load(Ordering::Relaxed)
    }

    pub fn take_change(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }
}

pub fn follows_jack(is_dsos: Option<&str>) -> bool {
    is_dsos == Some("1")
}
