use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use slot_gfx::{Compositor, TexId};
use slot_store::Cart;
use slot_ui::{cart_face_with, cart_interior, shows_interior, CartFace};

use crate::app::{App, FaceWant};
use crate::label_cache::label_art;

const WORKERS: usize = 2;

type Key = (usize, usize);

/// A cart's face and, for a clear GBA cart, the game-coloured inside it is drawn over.
type Built = (Key, CartFace, Option<CartFace>);

#[derive(Default)]
struct Queue {
    jobs: VecDeque<(Key, Cart)>,
    closed: bool,
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;

#[derive(Default)]
struct Building {
    carts: HashMap<Key, Cart>,
    outdated: HashSet<Key>,
}

impl Building {
    fn start(&mut self, key: Key, cart: &Cart) -> bool {
        match self.carts.get(&key) {
            None => {
                self.carts.insert(key, cart.clone());
                true
            }
            Some(building) => {
                if building != cart {
                    self.outdated.insert(key);
                }
                false
            }
        }
    }

    fn unqueue(&mut self, key: Key) {
        self.carts.remove(&key);
        self.outdated.remove(&key);
    }

    fn finish(&mut self, key: Key) -> bool {
        self.carts.remove(&key);
        !self.outdated.remove(&key)
    }
}

pub struct CartFaces {
    queue: Shared,
    built: Receiver<Built>,
    building: Building,
    asked: Vec<FaceWant>,
    free: Vec<TexId>,
}

impl CartFaces {
    pub fn spawn(root: PathBuf) -> Self {
        let queue: Shared = Arc::default();
        let (outbox, built) = mpsc::channel();
        for n in 0..WORKERS {
            let (queue, outbox, root) = (queue.clone(), outbox.clone(), root.clone());
            let spawned = thread::Builder::new()
                .name(format!("slot-carts-{n}"))
                .spawn(move || work(&queue, &outbox, &root));
            if let Err(e) = spawned {
                eprintln!("slot: cart faces: worker thread failed to start: {e}");
            }
        }
        CartFaces {
            queue,
            built,
            building: Building::default(),
            asked: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        self.free.extend(app.shed_faces());
        while let Ok((key, face, interior)) = self.built.try_recv() {
            self.place(app, compositor, key, face, interior);
        }
        let wants = app.face_wants();
        if wants != self.asked {
            self.requeue(app, &wants);
            self.asked = wants;
        }
        let mut waiting: Vec<Key> = self
            .asked
            .iter()
            .filter(|w| w.on_screen)
            .map(|w| (w.shelf, w.index))
            .collect();
        while !waiting.is_empty() {
            let Ok((key, face, interior)) = self.built.recv() else {
                return;
            };
            if self.place(app, compositor, key, face, interior) {
                waiting.retain(|k| *k != key);
            }
        }
    }

    fn requeue(&mut self, app: &App, wants: &[FaceWant]) {
        let (lock, wake) = &*self.queue;
        let mut queue = lock.lock().unwrap_or_else(|e| e.into_inner());
        for (key, _) in queue.jobs.drain(..) {
            self.building.unqueue(key);
        }
        for w in wants {
            let key = (w.shelf, w.index);
            if let Some(cart) = app.face_cart(w.shelf, w.index) {
                if self.building.start(key, cart) {
                    queue.jobs.push_back((key, cart.clone()));
                }
            }
        }
        wake.notify_all();
    }

    fn texture(&mut self, compositor: &mut Compositor, face: &CartFace) -> TexId {
        match self.free.pop() {
            Some(tex) => {
                compositor.update_texture(tex, face.w, face.h, &face.rgba);
                tex
            }
            None => compositor.create_texture(face.w, face.h, &face.rgba),
        }
    }

    /// A label can arrive, or a seal come or go, while a face is drawn, with nothing asking for
    /// the face again. Its cart is then drawn again as it is now, first in line.
    fn redraw_if_stale(&mut self, app: &App, key: Key, drawn: Option<&Cart>) -> bool {
        let Some(cart) = app.face_cart(key.0, key.1).filter(|c| Some(*c) != drawn) else {
            return false;
        };
        let (lock, wake) = &*self.queue;
        let mut queue = lock.lock().unwrap_or_else(|e| e.into_inner());
        if self.building.start(key, cart) {
            queue.jobs.push_front((key, cart.clone()));
        }
        wake.notify_all();
        true
    }

    /// What to do with a finished face. One drawn from an old cart is dropped, and the cart as
    /// it is now is drawn again first in line, so a frame waiting for it never waits on nothing.
    fn settle(&mut self, app: &App, key: Key) -> Settled {
        self.asked.retain(|w| (w.shelf, w.index) != key);
        let drawn = self.building.carts.get(&key).cloned();
        let current = self.building.finish(key);
        let drawn = drawn.filter(|_| current);
        if self.redraw_if_stale(app, key, drawn.as_ref()) {
            Settled::Redrawn
        } else if current {
            Settled::Place
        } else {
            Settled::Gone
        }
    }

    /// False while a newer face for the key is still being drawn.
    fn place(
        &mut self,
        app: &mut App,
        compositor: &mut Compositor,
        key: Key,
        face: CartFace,
        interior: Option<CartFace>,
    ) -> bool {
        match self.settle(app, key) {
            Settled::Redrawn => return false,
            Settled::Gone => return true,
            Settled::Place => {}
        }
        let tex = self.texture(compositor, &face);
        self.free.extend(app.set_cart_face(key.0, key.1, tex));
        if let Some(interior) = interior {
            let tex = self.texture(compositor, &interior);
            self.free.extend(app.set_cart_interior(key.0, key.1, tex));
        }
        true
    }
}

#[derive(Debug, PartialEq)]
enum Settled {
    Place,
    Redrawn,
    Gone,
}

impl Drop for CartFaces {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
        wake.notify_all();
    }
}

fn work(queue: &Shared, outbox: &Sender<Built>, root: &Path) {
    let (lock, wake) = &**queue;
    loop {
        let (key, cart) = {
            let mut q = lock.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if q.closed {
                    return;
                }
                if let Some(job) = q.jobs.pop_front() {
                    break job;
                }
                q = wake.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        slot_store::header_index::verify(&cart.rom);
        let face = cart_face_with(&cart, label_art(root, &cart));
        let interior = shows_interior(&cart).then(|| cart_interior(&cart));
        if outbox.send((key, face, interior)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_store::Platform;

    fn cart(stem: &str) -> Cart {
        Cart {
            platform: Platform::Gba,
            stem: stem.into(),
            rom: "a.gba".into(),
            label: None,
            title: stem.into(),
            code: String::new(),
            shell: None,
            sealed: false,
        }
    }

    #[test]
    fn a_face_built_from_an_old_cart_is_thrown_away() {
        let mut b = Building::default();
        let old = cart("A");
        let mut new = old.clone();
        new.label = Some("new.png".into());
        assert!(b.start((0, 0), &old));
        assert!(!b.start((0, 0), &new));
        assert!(!b.finish((0, 0)));
        assert!(b.start((0, 0), &new));
        assert!(b.finish((0, 0)));
    }

    #[test]
    fn the_same_cart_asked_twice_builds_once_and_lands() {
        let mut b = Building::default();
        assert!(b.start((0, 0), &cart("A")));
        assert!(!b.start((0, 0), &cart("A")));
        assert!(b.finish((0, 0)));
    }

    fn faces() -> CartFaces {
        let (_, built) = mpsc::channel();
        CartFaces {
            queue: Arc::default(),
            built,
            building: Building::default(),
            asked: Vec::new(),
            free: Vec::new(),
        }
    }

    fn queued(faces: &CartFaces) -> Vec<(Key, Cart)> {
        faces.queue.0.lock().unwrap().jobs.iter().cloned().collect()
    }

    #[test]
    fn a_face_drawn_before_the_cart_was_sealed_is_drawn_again() {
        let mut lone = cart("Sealed");
        lone.rom = "/x/Sealed.gba".into();
        let mut app = App::new(vec![lone]);
        let key = (app.face_wants()[0].shelf, 0);
        let mut faces = faces();
        let before = app.face_cart(key.0, key.1).unwrap().clone();
        app.seal(&HashSet::from([PathBuf::from("/x/Sealed.gba")]), false);
        assert!(faces.redraw_if_stale(&app, key, Some(&before)));
        let jobs = queued(&faces);
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].1.sealed, "the cart is drawn again as it is now");
        let now = app.face_cart(key.0, key.1).unwrap().clone();
        assert!(!faces.redraw_if_stale(&app, key, Some(&now)));
    }

    #[test]
    fn a_face_drawn_before_its_label_arrived_is_drawn_again() {
        let mut lone = cart("Labelled");
        lone.rom = "/x/Labelled.gba".into();
        let mut app = App::new(vec![lone]);
        let key = (app.face_wants()[0].shelf, 0);
        let mut faces = faces();
        let before = app.face_cart(key.0, key.1).unwrap().clone();
        app.attach_label(Path::new("/x/Labelled.gba"), "/l/Labelled.png".into());
        assert!(faces.redraw_if_stale(&app, key, Some(&before)));
        assert_eq!(
            queued(&faces)[0].1.label,
            Some(PathBuf::from("/l/Labelled.png"))
        );
    }

    #[test]
    fn a_face_outdated_while_drawn_is_drawn_again_so_a_wait_for_it_ends() {
        let mut lone = cart("Moved");
        lone.rom = "/x/Moved.gba".into();
        let mut app = App::new(vec![lone]);
        let key = (app.face_wants()[0].shelf, 0);
        let mut faces = faces();
        let old = app.face_cart(key.0, key.1).unwrap().clone();
        assert!(faces.building.start(key, &old));
        app.attach_label(Path::new("/x/Moved.gba"), "/l/Moved.png".into());
        let now = app.face_cart(key.0, key.1).unwrap().clone();
        assert!(!faces.building.start(key, &now), "marked outdated, not queued");
        assert!(queued(&faces).is_empty());
        assert_eq!(faces.settle(&app, key), Settled::Redrawn);
        let jobs = queued(&faces);
        assert_eq!(jobs.len(), 1, "the cart as it is now is queued");
        assert_eq!(jobs[0].1, now);
    }

    #[test]
    fn a_current_face_is_placed_and_a_face_for_a_cart_now_gone_is_dropped() {
        let mut lone = cart("Here");
        lone.rom = "/x/Here.gba".into();
        let app = App::new(vec![lone]);
        let key = (app.face_wants()[0].shelf, 0);
        let mut faces = faces();
        let cart = app.face_cart(key.0, key.1).unwrap().clone();
        assert!(faces.building.start(key, &cart));
        assert_eq!(faces.settle(&app, key), Settled::Place);
        let gone = (key.0, 99);
        assert!(faces.building.start(gone, &cart));
        assert!(!faces.building.start(gone, &self::cart("Other")));
        assert_eq!(faces.settle(&app, gone), Settled::Gone);
    }
}
