use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct Frames {
    inner: Mutex<Inner>,
    size: usize,
    taken: AtomicU64,
    /// Published frames overwritten before the renderer took them.
    dropped: AtomicU64,
    /// Calls to `latest` that found nothing new, so the renderer showed the previous picture
    /// again.
    repeated: AtomicU64,
}

struct Inner {
    ready: Option<Vec<u8>>,
    spare: Vec<Vec<u8>>,
    allocated: usize,
}

impl Frames {
    pub fn new(size: usize) -> Arc<Self> {
        Arc::new(Frames {
            inner: Mutex::new(Inner {
                ready: None,
                spare: Vec::new(),
                allocated: 0,
            }),
            size,
            taken: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            repeated: AtomicU64::new(0),
        })
    }

    pub fn take_write(&self) -> Vec<u8> {
        let mut i = self.lock();
        match i.spare.pop() {
            Some(buf) => buf,
            None => {
                i.allocated += 1;
                Vec::with_capacity(self.size)
            }
        }
    }

    pub fn publish(&self, buf: Vec<u8>) {
        let mut i = self.lock();
        if let Some(dropped) = i.ready.replace(buf) {
            i.spare.push(dropped);
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn latest(self: &Arc<Self>) -> Option<FrameRef> {
        let Some(buf) = self.lock().ready.take() else {
            self.repeated.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        self.taken.fetch_add(1, Ordering::Relaxed);
        Some(FrameRef {
            frames: self.clone(),
            buf,
        })
    }

    pub fn is_ready(&self) -> bool {
        self.lock().ready.is_some()
    }

    pub fn taken(&self) -> u64 {
        self.taken.load(Ordering::Relaxed)
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn repeated(&self) -> u64 {
        self.repeated.load(Ordering::Relaxed)
    }

    pub fn allocated(&self) -> usize {
        self.lock().allocated
    }

    fn recycle(&self, buf: Vec<u8>) {
        self.lock().spare.push(buf);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct FrameRef {
    frames: Arc<Frames>,
    buf: Vec<u8>,
}

impl Deref for FrameRef {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf
    }
}

impl Drop for FrameRef {
    fn drop(&mut self) {
        self.frames.recycle(std::mem::take(&mut self.buf));
    }
}
