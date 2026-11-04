use std::collections::VecDeque;
use std::path::PathBuf;

use slot_store::Platform;

use crate::art::jobs::ArtEvent;
use crate::art::make::Kind;
use crate::update::UpdateFail;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Arrive,
    Hold,
    Peel,
    Toss,
    Fly,
    Roll,
    Rest,
    Leave,
}

const STAGES: [(Stage, f64); 8] = [
    (Stage::Arrive, 160.0),
    (Stage::Hold, 80.0),
    (Stage::Peel, 420.0),
    (Stage::Toss, 260.0),
    (Stage::Fly, 420.0),
    (Stage::Roll, 380.0),
    (Stage::Rest, 450.0),
    (Stage::Leave, 130.0),
];

pub const SCENE_MS: f64 = 160.0 + 80.0 + 420.0 + 260.0 + 420.0 + 380.0 + 450.0 + 130.0;

pub fn stage_start(stage: Stage) -> f64 {
    STAGES.iter().take_while(|(s, _)| *s != stage).map(|(_, ms)| ms).sum()
}

pub fn stage_at(elapsed_ms: f64) -> (Stage, f32) {
    let mut t = elapsed_ms.max(0.0);
    for (stage, ms) in STAGES {
        if t < ms {
            return (stage, (t / ms) as f32);
        }
        t -= ms;
    }
    (Stage::Leave, 1.0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Landed {
    pub stem: String,
    pub platform: Platform,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub cart: Landed,
    pub number: usize,
    pub started_ms: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub kind: Option<Kind>,
    pub counted: bool,
    pub total: usize,
    pub played: usize,
    pub queue: VecDeque<Landed>,
    pub scene: Option<Scene>,
    pub landed: usize,
    pub carts: Vec<Landed>,
    pub missed: Vec<String>,
    pub finished: bool,
    pub stopping: bool,
    pub skipping: bool,
    pub closing: Option<(Landed, f64)>,
    pub failed: Option<UpdateFail>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FetchArt {
    Choice { pick: Kind, missing: usize },
    Running(Run),
    Summary {
        featured: Option<Landed>,
        stopped: bool,
        landed: usize,
        missed: Vec<String>,
        failed: Option<UpdateFail>,
    },
}

impl Run {
    pub fn new(kind: Kind) -> Run {
        Run {
            kind: Some(kind),
            ..Run::default()
        }
    }

    pub fn event(&mut self, event: ArtEvent) {
        match event {
            ArtEvent::Counted(n) => {
                self.counted = true;
                self.total = n;
            }
            ArtEvent::Landed {
                stem,
                platform,
                path,
            } => {
                self.landed += 1;
                let cart = Landed {
                    stem,
                    platform,
                    path,
                };
                self.carts.push(cart.clone());
                self.queue.push_back(cart);
            }
            ArtEvent::Missed { stem } => self.missed.push(stem),
            ArtEvent::Done { .. } => self.finished = true,
            ArtEvent::StudioArt(_) | ArtEvent::StudioSaved(_) => {}
            ArtEvent::Failed(f) => {
                self.failed = Some(f);
                self.finished = true;
            }
        }
    }

    pub fn skip(&mut self) -> Vec<Landed> {
        self.skipping = true;
        let current = self.scene.take().map(|s| s.cart);
        current.into_iter().chain(self.queue.drain(..)).collect()
    }

    pub fn stop(&mut self) -> Vec<Landed> {
        self.stopping = true;
        self.queue.drain(..).collect()
    }

    pub fn tick(&mut self, now_ms: f64, ready: &dyn Fn(&Landed) -> bool) -> Vec<Landed> {
        let mut done = Vec::new();
        if let Some(scene) = &self.scene {
            if now_ms - scene.started_ms >= SCENE_MS {
                done.push(scene.cart.clone());
                self.scene = None;
            }
        }
        if self.stopping || self.skipping {
            done.extend(self.queue.drain(..));
        }
        if self.scene.is_none() && self.queue.front().is_some_and(ready) {
            if let Some(cart) = self.queue.pop_front() {
                self.played += 1;
                self.scene = Some(Scene {
                    cart,
                    number: self.played,
                    started_ms: now_ms,
                });
            }
        }
        done
    }

    pub fn over(&self) -> bool {
        self.finished && self.queue.is_empty() && self.scene.is_none()
    }

    pub fn drop_unplayable(&mut self, playable: &dyn Fn(&Landed) -> bool) -> Vec<Landed> {
        let (keep, gone): (VecDeque<Landed>, VecDeque<Landed>) =
            self.queue.drain(..).partition(|l| playable(l));
        self.queue = keep;
        gone.into_iter().collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FetchTex {
    pub title: slot_ui::TexId,
    pub bare: slot_ui::TexId,
    pub done: slot_ui::TexId,
    pub under: Option<[slot_ui::TexId; 2]>,
    pub w: u32,
    pub h: u32,
}

pub fn choice_xs(canvas_w: f32) -> [f32; 2] {
    let mid = canvas_w / 2.0;
    [mid - 304.0, mid + 16.0]
}
