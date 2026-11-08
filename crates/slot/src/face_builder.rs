use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use slot_store::{Cart, Platform};
use slot_ui::{board_face, cart_face_with, cart_interior, padded, CartFace, TURN_PAD};

use crate::label_cache::label_art;

pub struct BuiltFaces {
    pub label: Option<std::path::PathBuf>,
    pub platform: Platform,
    pub stem: String,
    pub sealed: bool,
    pub interior: CartFace,
    pub board: CartFace,
    pub lid: CartFace,
}

impl BuiltFaces {
    pub fn is_for(&self, cart: &Cart) -> bool {
        self.stem == cart.stem && self.label == cart.label && self.sealed == cart.sealed
    }
}

pub struct FaceBuilder {
    requests: Sender<Cart>,
    built: Receiver<BuiltFaces>,
}

impl FaceBuilder {
    pub fn spawn(root: PathBuf) -> Self {
        let (requests, inbox) = mpsc::channel::<Cart>();
        let (outbox, built) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-faces".into())
            .spawn(move || {
                while let Ok(mut cart) = inbox.recv() {
                    while let Ok(newer) = inbox.try_recv() {
                        cart = newer;
                    }
                    let faces = BuiltFaces {
                        platform: cart.platform,
                        stem: cart.stem.clone(),
                        label: cart.label.clone(),
                        sealed: cart.sealed,
                        interior: cart_interior(&cart),
                        board: board_face(&cart),
                        lid: padded(&cart_face_with(&cart, label_art(&root, &cart)), TURN_PAD),
                    };
                    if outbox.send(faces).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: faces: worker thread failed to start: {e}");
        }
        FaceBuilder { requests, built }
    }

    pub fn request(&self, cart: Cart) {
        let _ = self.requests.send(cart);
    }

    pub fn take(&self) -> Option<BuiltFaces> {
        let mut newest = None;
        while let Ok(faces) = self.built.try_recv() {
            newest = Some(faces);
        }
        newest
    }
}
