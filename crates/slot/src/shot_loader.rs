use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Want {
    Thumb,
    Full,
}

pub struct Loaded {
    pub path: PathBuf,
    pub want: Want,
    pub generation: u64,
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

struct Job {
    path: PathBuf,
    want: Want,
    generation: u64,
    max: (u32, u32),
}

#[derive(Default)]
struct Queue {
    full: Option<Job>,
    thumbs: VecDeque<Job>,
    generation: u64,
    working: Option<PathBuf>,
    closed: bool,
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;

pub struct ShotLoader {
    shared: Shared,
    done: Receiver<Loaded>,
    decoded: Arc<AtomicUsize>,
}

impl ShotLoader {
    pub fn spawn() -> Self {
        let shared: Shared = Arc::default();
        let (tx, done) = channel();
        let worker = shared.clone();
        let decoded = Arc::new(AtomicUsize::new(0));
        let count = decoded.clone();
        if let Err(e) = thread::Builder::new()
            .name("slot-shot-load".into())
            .spawn(move || work(&worker, &tx, &count))
        {
            eprintln!("slot: gallery: no loader thread: {e}");
        }
        ShotLoader {
            shared,
            done,
            decoded,
        }
    }

    fn queue(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn queue_thumbs(&mut self, paths: Vec<PathBuf>, max_w: u32, max_h: u32) {
        let mut q = self.queue();
        let generation = q.generation;
        let working = q.working.clone();
        q.thumbs = paths
            .into_iter()
            .filter(|p| Some(p) != working.as_ref())
            .map(|path| Job {
                path,
                want: Want::Thumb,
                generation,
                max: (max_w, max_h),
            })
            .collect();
        drop(q);
        self.shared.1.notify_all();
    }

    pub fn full(&mut self, path: PathBuf) {
        let mut q = self.queue();
        q.full = Some(Job {
            path,
            want: Want::Full,
            generation: q.generation,
            max: (0, 0),
        });
        drop(q);
        self.shared.1.notify_all();
    }

    pub fn cancel(&mut self) -> u64 {
        let mut q = self.queue();
        q.generation += 1;
        q.full = None;
        q.thumbs.clear();
        q.working = None;
        q.generation
    }

    pub fn decoded(&self) -> usize {
        self.decoded.load(Ordering::Relaxed)
    }

    pub fn poll(&self) -> Vec<Loaded> {
        let generation = self.queue().generation;
        self.done
            .try_iter()
            .filter(|l| l.generation == generation)
            .collect()
    }
}

impl Drop for ShotLoader {
    fn drop(&mut self) {
        self.queue().closed = true;
        self.shared.1.notify_all();
    }
}

fn work(shared: &Shared, done: &Sender<Loaded>, decoded: &AtomicUsize) {
    let (lock, wake) = &**shared;
    loop {
        let job = {
            let mut q = lock.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if q.closed {
                    return;
                }
                if let Some(job) = q.full.take().or_else(|| q.thumbs.pop_front()) {
                    q.working = Some(job.path.clone());
                    break job;
                }
                q = wake.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        let decoded_png = slot_ui::decode_png(&job.path);
        decoded.fetch_add(1, Ordering::Relaxed);
        lock.lock().unwrap_or_else(|e| e.into_inner()).working = None;
        let Some((rgba, w, h)) = decoded_png else {
            eprintln!("slot: shot decode: {}", job.path.display());
            continue;
        };
        let (rgba, w, h) = match job.want {
            Want::Full => (rgba, w, h),
            Want::Thumb => slot_ui::thumbnail(&rgba, w, h, job.max.0, job.max.1),
        };
        let loaded = Loaded {
            path: job.path,
            want: job.want,
            generation: job.generation,
            rgba,
            w,
            h,
        };
        if done.send(loaded).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopening_can_request_a_thumbnail_still_decoding_from_the_previous_generation() {
        let (_tx, done) = channel();
        let path = PathBuf::from("shot.png");
        let shared = Arc::new((
            Mutex::new(Queue {
                working: Some(path.clone()),
                ..Queue::default()
            }),
            Condvar::new(),
        ));
        let mut loader = ShotLoader {
            shared,
            done,
            decoded: Arc::default(),
        };
        let generation = loader.cancel();
        loader.queue_thumbs(vec![path.clone()], 218, 145);
        let queue = loader.queue();
        let job = queue
            .thumbs
            .front()
            .expect("reopened thumbnail was dropped");
        assert_eq!(job.path, path);
        assert_eq!(job.generation, generation);
    }
}
