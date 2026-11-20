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
    /// Where the chip is along the row, counted in seats from the first.
    pub across: f32,
    pub lift: f32,
    pub tip: f32,
    pub seated: Option<Core>,
    pub shake: f32,
}

#[derive(Copy, Clone)]
enum Motion {
    Seated(usize),
    Hopping(Hop),
}

/// A hop to a neighbouring seat, and the seat after it that a press made in flight asked for.
#[derive(Copy, Clone)]
struct Hop {
    from: usize,
    to: usize,
    started: Millis,
    then: Option<usize>,
}

impl Hop {
    fn rightward(&self) -> bool {
        self.to > self.from
    }

    fn progress(&self, now: Millis) -> f32 {
        now.saturating_sub(self.started) as f32 / HOP_MS as f32
    }
}

#[derive(Copy, Clone)]
struct Close {
    started: Millis,
    from: f32,
}

#[derive(Copy, Clone)]
pub struct CorePicker {
    row: &'static [Core],
    motion: Motion,
    opened: Millis,
    started: Option<Millis>,
    close: Option<Close>,
    refusal: Option<Refusal>,
}

impl CorePicker {
    pub fn open(row: &'static [Core], seat: Core, now: Millis) -> Self {
        let at = row.iter().position(|c| *c == seat).unwrap_or(0);
        CorePicker {
            row,
            motion: Motion::Seated(at),
            opened: now,
            started: None,
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

    /// The core the chip ends up in once it stops hopping.
    pub fn seat(&self) -> Core {
        self.row[match self.motion {
            Motion::Seated(seat) => seat,
            Motion::Hopping(hop) => hop.then.unwrap_or(hop.to),
        }]
    }

    pub fn row(&self) -> &'static [Core] {
        self.row
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
        let rightward = match press {
            Press::Keep => {
                self.begin_close(now);
                return Outcome::Write(self.seat());
            }
            Press::Back => {
                self.begin_close(now);
                return Outcome::Nothing;
            }
            Press::Left => false,
            Press::Right => true,
        };
        if self.waiting() {
            return Outcome::Nothing;
        }
        self.motion = self.motion_at(now);
        let seats = self.row.len();
        let beside = |seat: usize| match rightward {
            true => Some(seat + 1).filter(|s| *s < seats),
            false => seat.checked_sub(1),
        };
        self.motion = match self.motion {
            Motion::Seated(seat) => {
                let Some(to) = beside(seat) else {
                    self.refusal = Some(Refusal::started(now));
                    return Outcome::Refused;
                };
                self.refusal = None;
                Motion::Hopping(Hop {
                    from: seat,
                    to,
                    started: now,
                    then: None,
                })
            }
            Motion::Hopping(hop) if hop.rightward() == rightward => Motion::Hopping(Hop {
                then: hop.then.or_else(|| beside(hop.to)),
                ..hop
            }),
            Motion::Hopping(hop) if hop.then.is_some() => {
                Motion::Hopping(Hop { then: None, ..hop })
            }
            Motion::Hopping(hop) => {
                let left = ((1.0 - hop.progress(now)) * HOP_MS as f32) as Millis;
                self.refusal = None;
                Motion::Hopping(Hop {
                    from: hop.to,
                    to: hop.from,
                    started: now.saturating_sub(left),
                    then: None,
                })
            }
        };
        Outcome::Nothing
    }

    pub fn chip(&self, now: Millis) -> Chip {
        let shake = self.refusal.map_or(0.0, |r| r.offset(now));
        match self.motion_at(now) {
            Motion::Hopping(hop) => {
                let q = hop.progress(now);
                let lean = if hop.rightward() { 1.0 } else { -1.0 };
                let arc = (std::f32::consts::PI * q).sin();
                Chip {
                    across: hop.from as f32 + lean * ease(q),
                    lift: arc,
                    tip: lean * CHIP_TIP * arc,
                    seated: None,
                    shake,
                }
            }
            Motion::Seated(seat) => Chip {
                across: seat as f32,
                lift: 0.0,
                tip: 0.0,
                seated: Some(self.row[seat]),
                shake,
            },
        }
    }

    /// The motion at `now`: a hop that has landed is in its seat, or already on the hop asked for
    /// while it flew.
    fn motion_at(&self, now: Millis) -> Motion {
        let mut motion = self.motion;
        while let Motion::Hopping(hop) = motion {
            if hop.progress(now) < 1.0 {
                break;
            }
            motion = match hop.then {
                Some(to) => Motion::Hopping(Hop {
                    from: hop.to,
                    to,
                    started: hop.started + HOP_MS,
                    then: None,
                }),
                None => Motion::Seated(hop.to),
            };
        }
        motion
    }

    fn begin_close(&mut self, now: Millis) {
        self.close = Some(Close {
            started: now,
            from: self.openness(now),
        });
    }
}
