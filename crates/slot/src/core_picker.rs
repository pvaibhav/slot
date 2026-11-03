use slot_store::Core;
use slot_ui::{ease, Millis, Refusal, CHIP_TIP};

pub const SLIDE_MS: Millis = 160;
pub const LIFT_MS: Millis = 260;
pub const OPEN_MS: Millis = SLIDE_MS + LIFT_MS;
pub const CLOSE_MS: Millis = 320;
pub const HOP_MS: Millis = 180;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Press {
    Left,
    Right,
    Keep,
    Back,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Nothing,
    Refused,
    Write(Core),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Chip {
    pub across: f32,
    pub lift: f32,
    pub tip: f32,
    pub seated: Option<Core>,
    pub shake: f32,
}

#[derive(Copy, Clone)]
struct Hop {
    from: Core,
    started: Millis,
}

#[derive(Copy, Clone)]
struct Close {
    started: Millis,
    from: f32,
}

#[derive(Copy, Clone)]
pub struct CorePicker {
    pair: [Core; 2],
    seat: Core,
    opened: Millis,
    started: Option<Millis>,
    hop: Option<Hop>,
    close: Option<Close>,
    refusal: Option<Refusal>,
}

impl CorePicker {
    pub fn open(pair: [Core; 2], seat: Core, now: Millis) -> Self {
        CorePicker {
            pair,
            seat,
            opened: now,
            started: None,
            hop: None,
            close: None,
            refusal: None,
        }
    }

    pub fn start(&mut self, now: Millis) {
        self.started.get_or_insert(now);
    }

    pub fn waiting(&self) -> bool {
        self.started.is_none()
    }

    pub fn waited(&self, now: Millis) -> Millis {
        now.saturating_sub(self.opened)
    }

    pub fn seat(&self) -> Core {
        self.seat
    }

    pub fn pair(&self) -> [Core; 2] {
        self.pair
    }

    pub fn closing(&self) -> bool {
        self.close.is_some()
    }

    pub fn openness(&self, now: Millis) -> f32 {
        match (self.close, self.started) {
            (Some(Close { started, from }), _) => {
                (from - now.saturating_sub(started) as f32 / CLOSE_MS as f32).max(0.0)
            }
            (None, Some(started)) => (now.saturating_sub(started) as f32 / OPEN_MS as f32).min(1.0),
            (None, None) => 0.0,
        }
    }

    pub fn finished(&self, now: Millis) -> bool {
        self.close.is_some() && self.openness(now) <= 0.0
    }

    pub fn press(&mut self, press: Press, now: Millis) -> Outcome {
        if self.close.is_some() {
            return Outcome::Nothing;
        }
        let target = match press {
            Press::Keep => {
                self.begin_close(now);
                return Outcome::Write(self.seat);
            }
            Press::Back => {
                self.begin_close(now);
                return Outcome::Nothing;
            }
            Press::Left => self.pair[0],
            Press::Right => self.pair[1],
        };
        if self.waiting() {
            return Outcome::Nothing;
        }
        if let Some(progress) = self.hop_progress(now) {
            if target == self.seat {
                return Outcome::Nothing;
            }
            let done = ((1.0 - progress) * HOP_MS as f32) as Millis;
            self.hop = Some(Hop {
                from: self.seat,
                started: now.saturating_sub(done),
            });
            self.seat = target;
            self.refusal = None;
            return Outcome::Nothing;
        }
        if target == self.seat {
            self.refusal = Some(Refusal::started(now));
            return Outcome::Refused;
        }
        self.hop = Some(Hop {
            from: self.seat,
            started: now,
        });
        self.seat = target;
        self.refusal = None;
        Outcome::Nothing
    }

    pub fn chip(&self, now: Millis) -> Chip {
        let shake = self.refusal.map_or(0.0, |r| r.offset(now));
        match (self.hop, self.hop_progress(now)) {
            (Some(hop), Some(q)) => {
                let rightward = hop.from == self.pair[0];
                let lean = if rightward { 1.0 } else { -1.0 };
                let arc = (std::f32::consts::PI * q).sin();
                Chip {
                    across: if rightward { ease(q) } else { 1.0 - ease(q) },
                    lift: arc,
                    tip: lean * CHIP_TIP * arc,
                    seated: None,
                    shake,
                }
            }
            _ => Chip {
                across: self.place(self.seat) as f32,
                lift: 0.0,
                tip: 0.0,
                seated: Some(self.seat),
                shake,
            },
        }
    }

    fn place(&self, core: Core) -> usize {
        usize::from(core == self.pair[1])
    }

    fn begin_close(&mut self, now: Millis) {
        self.close = Some(Close {
            started: now,
            from: self.openness(now),
        });
    }

    fn hop_progress(&self, now: Millis) -> Option<f32> {
        let hop = self.hop?;
        let q = now.saturating_sub(hop.started) as f32 / HOP_MS as f32;
        (q < 1.0).then_some(q)
    }
}
