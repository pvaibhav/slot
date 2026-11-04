use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use slot_store::{Cart, Platform};
use slot_ui::{
    bare_face, cart_backing, cart_face, cart_face_with, cart_interior, cover_bytes, gb_shell_of,
    label_size, shows_interior, CartFace,
};

pub struct FaceSet {
    pub title: CartFace,
    pub bare: CartFace,
    pub done: CartFace,
    pub under: Option<[CartFace; 2]>,
}

pub enum FaceJob {
    Cart(Cart),
    Examples,
}

pub enum FaceDone {
    Cart(Platform, String, FaceSet),
    Examples([CartFace; 2]),
}

pub struct FetchFaces {
    jobs: Sender<FaceJob>,
    done: Receiver<FaceDone>,
}

pub fn faces_for(cart: &Cart) -> FaceSet {
    let mut untitled = cart.clone();
    untitled.label = None;
    FaceSet {
        title: cart_face_with(&untitled, None),
        bare: bare_face(cart),
        done: cart_face(cart),
        under: shows_interior(cart)
            .then(|| [cart_interior(cart), cart_backing(gb_shell_of(cart))]),
    }
}

const EXAMPLE_REAL: &[u8] = include_bytes!("../../../assets/fetch_example_real.png");
const EXAMPLE_LOGO: &[u8] = include_bytes!("../../../assets/fetch_example_logo.png");

pub fn example_faces() -> [CartFace; 2] {
    let cart = Cart {
        platform: Platform::Gba,
        stem: "Apotris".into(),
        rom: PathBuf::from("Apotris.gba"),
        label: None,
        title: "APOTRIS".into(),
        code: String::new(),
        shell: None,
        sealed: false,
    };
    let (w, h) = label_size(&cart);
    [EXAMPLE_REAL, EXAMPLE_LOGO].map(|png| cart_face_with(&cart, cover_bytes(png, w, h)))
}

impl FetchFaces {
    pub fn spawn() -> FetchFaces {
        let (jobs, rx) = channel::<FaceJob>();
        let (tx, done) = channel::<FaceDone>();
        let spawned = std::thread::Builder::new().name("slot-fetch-faces".into()).spawn(move || {
            for job in rx {
                let out = match job {
                    FaceJob::Examples => FaceDone::Examples(example_faces()),
                    FaceJob::Cart(cart) => {
                        FaceDone::Cart(cart.platform, cart.stem.clone(), faces_for(&cart))
                    }
                };
                if tx.send(out).is_err() {
                    return;
                }
            }
        });
        if let Err(e) = spawned {
            eprintln!("slot: fetch faces: worker thread failed to start: {e}");
        }
        FetchFaces { jobs, done }
    }

    pub fn ask(&self, job: FaceJob) {
        let _ = self.jobs.send(job);
    }

    pub fn take(&self) -> Option<FaceDone> {
        self.done.try_recv().ok()
    }
}
