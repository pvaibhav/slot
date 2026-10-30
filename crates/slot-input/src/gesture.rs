use crate::{Btn, Millis, RawEvent};

pub const SELECT_CHORD_MS: Millis = 600;

pub const SELECT_TAP_MS: Millis = 50;
pub const MENU_TAP_MS: Millis = 250;
pub const MENU_DOUBLE_TAP_MS: Millis = 350;
pub const MENU_HOLD_MS: Millis = 1000;
pub const FF_DOUBLE_TAP_MS: Millis = 250;
pub const MUTE_CHORD_MS: Millis = 200;
pub const POWER_HOLD_MS: Millis = 1000;

pub const VOLUME_REPEAT_DELAY_MS: Millis = 400;
pub const VOLUME_REPEAT_MS: Millis = 120;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    SelectTap,
    GbaDown(Btn),
    GbaUp(Btn),
    ShelfLeft,
    ShelfRight,
    Insert,
    Eject,
    Polaroids,
    SaveState,
    LoadState,
    Screenshot,
    RewindStart,
    RewindStop,
    FfStart,
    FfStop,
    FfToggle,
    BrightnessUp,
    BrightnessDown,
    BlueLightUp,
    BlueLightDown,
    SunlightUp,
    SunlightDown,
    Reset,
    Guide,
    VolumeUp,
    VolumeDown,
    QuickMenu,
    GameMenu,
    MuteToggle,
    ColourCorrectionToggle,
    PaletteNext,
    PalettePrev,
    ShaderNext,
    ShaderPrev,
    PowerPress,
    PowerTap,
    PowerHold,
    PowerOff,
    LidClose,
    LidOpen,
    MenuChordDown(Btn),
    MenuChordUp(Btn),
    RepairCard,
    Restart,
    PreviousShelf,
    NextShelf,
    Favorite,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Context {
    #[default]
    Game,
    Shelf,
    Menu,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Contexts(u8);

impl Contexts {
    pub const GAME: Contexts = Contexts(1);
    pub const SHELF: Contexts = Contexts(2);
    pub const MENU: Contexts = Contexts(4);
    pub const ALL: Contexts = Contexts(7);

    pub fn with(self, other: Contexts) -> Contexts {
        Contexts(self.0 | other.0)
    }

    pub fn has(self, context: Context) -> bool {
        let bit = match context {
            Context::Game => Self::GAME,
            Context::Shelf => Self::SHELF,
            Context::Menu => Self::MENU,
        };
        self.0 & bit.0 != 0
    }

    pub fn meets(self, other: Contexts) -> bool {
        self.0 & other.0 != 0
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    pub chord: Chord,
    pub action: Action,
    pub contexts: Contexts,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mod {
    Select,
    Menu,
    Button(Btn),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Chord {
    pub modifier: Option<Mod>,
    pub button: Btn,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Release {
    Nothing,
    Send(Action),
}

#[derive(Copy, Clone, Default)]
enum Select {
    #[default]
    Idle,
    Held {
        since: Millis,
        chorded: bool,
        sent: Option<Millis>,
    },
    ReleaseDue(Millis),
}

#[derive(Default)]
pub struct Gestures {
    select: Select,
    entries: Vec<Entry>,
    context: Context,
    pressed: Vec<Btn>,
    deferred: Vec<Btn>,
    consumed: Vec<Btn>,
    held: Vec<(Btn, Release)>,
    ff_button: Option<Btn>,
    rewind_button: Option<Btn>,
    menu_down_at: Option<Millis>,
    menu_last_tap: Option<Millis>,
    menu_eject_fired: bool,
    menu_chorded: bool,
    power_down_at: Option<Millis>,
    power_hold_fired: bool,
    vol_up_at: Option<Millis>,
    vol_down_at: Option<Millis>,
    vol_up_ramp: Option<Millis>,
    vol_down_ramp: Option<Millis>,
    mute_fired: bool,
    ff_on: bool,
    ff_latched: bool,
    ff_latching_press: bool,
    r2_refused: bool,
    r2_last_release: Option<Millis>,
    rewinding: bool,
}

pub fn anywhere(action: Action) -> bool {
    matches!(
        action,
        Action::BrightnessUp | Action::BrightnessDown | Action::BlueLightUp | Action::BlueLightDown
    )
}

fn leads(b: Btn) -> bool {
    matches!(b, Btn::Select | Btn::Start | Btn::Menu)
}

fn ramp_due(down: Option<Millis>, last: Option<Millis>, now: Millis) -> bool {
    let Some(down) = down else {
        return false;
    };
    if now.saturating_sub(down) < VOLUME_REPEAT_DELAY_MS {
        return false;
    }
    last.is_none_or(|l| now.saturating_sub(l) >= VOLUME_REPEAT_MS)
}

impl Gestures {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_shortcuts(&mut self, shortcuts: Vec<(Chord, Action)>) {
        self.entries = shortcuts
            .into_iter()
            .map(|(chord, action)| Entry {
                chord,
                action,
                contexts: match (chord.modifier, anywhere(action)) {
                    (Some(Mod::Select | Mod::Menu), _) => Contexts::ALL,
                    (Some(Mod::Button(_)), true) => Contexts::ALL,
                    _ => Contexts::GAME,
                },
            })
            .collect();
    }

    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
    }

    pub fn set_playing(&mut self, playing: bool) {
        self.set_context(match playing {
            true => Context::Game,
            false => Context::Menu,
        });
    }

    pub fn set_context(&mut self, context: Context) {
        if context != self.context {
            self.deferred.clear();
        }
        self.context = context;
    }

    pub fn observe(&mut self, ev: RawEvent) {
        match ev {
            RawEvent::Down(b) if !self.pressed.contains(&b) => self.pressed.push(b),
            RawEvent::Down(_) => {}
            RawEvent::Up(b) => {
                self.pressed.retain(|x| *x != b);
                self.deferred.retain(|x| *x != b);
                self.consumed.retain(|x| *x != b);
                self.held.retain(|(h, _)| *h != b);
            }
        }
    }

    fn active(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.contexts.has(self.context))
    }

    fn matches(&self, modifier: Option<Mod>, button: Btn) -> Vec<Action> {
        let chord = Chord { modifier, button };
        self.active()
            .filter(|e| e.chord == chord)
            .map(|e| e.action)
            .collect()
    }

    fn shortcut(&self, modifier: Option<Mod>, button: Btn) -> Option<Action> {
        self.matches(modifier, button).first().copied()
    }

    fn starts_a_chord(&self, b: Btn) -> bool {
        self.active().any(|e| match e.chord.modifier {
            Some(Mod::Button(first)) => first == b || (e.chord.button == b && !leads(first)),
            _ => false,
        })
    }

    fn hold(&mut self, b: Btn, action: Action) -> Vec<Action> {
        let release = match action {
            Action::FfStart => Release::Send(Action::FfStop),
            Action::RewindStart => Release::Send(Action::RewindStop),
            _ => Release::Nothing,
        };
        self.held.push((b, release));
        vec![action]
    }

    fn menu_held(&self) -> bool {
        !self.menu_eject_fired && (self.menu_down_at.is_some() || self.menu_chorded)
    }

    pub fn ff_latched(&self) -> bool {
        self.ff_latched
    }

    pub fn drop_ff_latch(&mut self) -> Vec<Action> {
        if !self.ff_latched {
            return Vec::new();
        }
        self.ff_clear()
    }

    pub fn feed(&mut self, ev: RawEvent, now: Millis) -> Vec<Action> {
        match ev {
            RawEvent::Down(b) => {
                let out = self.down(b, now);
                if !self.pressed.contains(&b) {
                    self.pressed.push(b);
                }
                out
            }
            RawEvent::Up(b) => {
                self.pressed.retain(|x| *x != b);
                self.up(b, now)
            }
        }
    }

    pub fn tick(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if let Select::ReleaseDue(due) = self.select {
            if now >= due {
                self.select = Select::Idle;
                out.push(Action::GbaUp(Btn::Select));
            }
        }
        if let Select::Held {
            since,
            chorded: false,
            sent: None,
        } = self.select
        {
            if now.saturating_sub(since) >= SELECT_CHORD_MS {
                out.extend(self.hand_over_select(now));
            }
        }
        if let Some(d) = self.menu_down_at {
            if !self.menu_eject_fired && now.saturating_sub(d) >= MENU_HOLD_MS {
                self.menu_eject_fired = true;
                out.push(Action::Eject);
            }
        }
        if let Some(d) = self.power_down_at {
            if !self.power_hold_fired && now.saturating_sub(d) >= POWER_HOLD_MS {
                self.power_hold_fired = true;
                out.push(Action::PowerHold);
            }
        }
        if !self.mute_fired {
            if ramp_due(self.vol_up_at, self.vol_up_ramp, now) {
                self.vol_up_ramp = Some(now);
                out.push(Action::VolumeUp);
            }
            if ramp_due(self.vol_down_at, self.vol_down_ramp, now) {
                self.vol_down_ramp = Some(now);
                out.push(Action::VolumeDown);
            }
        }
        out
    }

    fn down(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        if !matches!(b, Btn::Menu | Btn::Select) && self.menu_held() {
            self.menu_chorded = true;
            self.menu_down_at = None;
            self.menu_last_tap = None;
            self.menu_eject_fired = false;
            return match self.shortcut(Some(Mod::Menu), b) {
                Some(action) => self.hold(b, action),
                None => {
                    self.held.push((b, Release::Send(Action::MenuChordUp(b))));
                    vec![Action::MenuChordDown(b)]
                }
            };
        }
        if self.chording(now) {
            let action = match b {
                Btn::VolUp | Btn::VolDown => Some(Action::MuteToggle),
                _ => self.shortcut(Some(Mod::Select), b),
            };
            if let Some(action) = action {
                self.mark_chorded();
                return self.hold(b, action);
            }
        }
        if let Some((first, action)) = self.pair(b) {
            if let Some(at) = self.deferred.iter().position(|x| *x == first) {
                self.deferred.remove(at);
                self.consumed.push(first);
            }
            return self.fire(b, action, now);
        }
        let system = matches!(
            b,
            Btn::Select | Btn::Menu | Btn::Power | Btn::Lid | Btn::VolUp | Btn::VolDown
        );
        if self.context == Context::Game {
            if let Some(action) = self.shortcut(None, b) {
                return self.fire(b, action, now);
            }
        } else if !system {
            if self.starts_a_chord(b) {
                self.deferred.push(b);
                return Vec::new();
            }
            let actions = self.matches(None, b);
            if !actions.is_empty() {
                let mut out = self.hand_over_select(now);
                out.extend(actions);
                out.push(Action::GbaDown(b));
                return out;
            }
        }
        match b {
            Btn::Select => self.select_down(now),
            Btn::Menu => self.menu_down(now),
            Btn::Power => {
                self.power_down_at = Some(now);
                self.power_hold_fired = false;
                vec![Action::PowerPress]
            }
            Btn::Lid => vec![Action::LidClose],
            Btn::VolUp | Btn::VolDown => self.volume_press(b, now),
            _ => {
                let mut out = self.hand_over_select(now);
                out.push(Action::GbaDown(b));
                out
            }
        }
    }

    fn up(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        if let Some(at) = self.consumed.iter().position(|x| *x == b) {
            self.consumed.remove(at);
            return Vec::new();
        }
        if let Some(at) = self.deferred.iter().position(|x| *x == b) {
            self.deferred.remove(at);
            let mut out = self.hand_over_select(now);
            out.extend(self.matches(None, b));
            out.extend([Action::GbaDown(b), Action::GbaUp(b)]);
            return out;
        }
        if let Some(at) = self.held.iter().position(|(h, _)| *h == b) {
            return match self.held.remove(at).1 {
                Release::Nothing => Vec::new(),
                Release::Send(action) => vec![action],
            };
        }
        if self.ff_button == Some(b) && (self.ff_on || self.r2_refused) {
            return self.ff_up(now);
        }
        if self.rewind_button == Some(b) && self.rewinding {
            return self.rewind_stop();
        }
        match b {
            Btn::Select => self.select_up(now),
            Btn::Menu => self.menu_up(now),
            Btn::Power => self.power_up(),
            Btn::Lid => vec![Action::LidOpen],
            Btn::VolUp | Btn::VolDown => self.volume_release(b),
            _ => vec![Action::GbaUp(b)],
        }
    }

    fn pair(&self, b: Btn) -> Option<(Btn, Action)> {
        if leads(b) {
            return None;
        }
        self.pressed
            .iter()
            .filter(|h| **h != b && !self.busy(**h))
            .find_map(|h| {
                self.shortcut(Some(Mod::Button(*h)), b)
                    .or_else(|| self.shortcut(Some(Mod::Button(b)), *h))
                    .map(|action| (*h, action))
            })
    }

    fn busy(&self, b: Btn) -> bool {
        self.held.iter().any(|(h, _)| *h == b)
            || (self.ff_button == Some(b) && self.ff_on)
            || (self.rewind_button == Some(b) && self.rewinding)
    }

    fn fire(&mut self, b: Btn, action: Action, now: Millis) -> Vec<Action> {
        match action {
            Action::FfStart => {
                self.ff_button = Some(b);
                self.ff_down(now)
            }
            Action::FfToggle => {
                self.held.push((b, Release::Nothing));
                self.ff_toggle()
            }
            Action::RewindStart => {
                self.rewind_button = Some(b);
                self.rewind_start()
            }
            action => self.hold(b, action),
        }
    }

    fn chording(&self, now: Millis) -> bool {
        match self.select {
            Select::Held { since, chorded, .. } => {
                chorded || now.saturating_sub(since) < SELECT_CHORD_MS
            }
            _ => false,
        }
    }

    fn mark_chorded(&mut self) {
        if let Select::Held { chorded, .. } = &mut self.select {
            *chorded = true;
        }
    }

    fn select_down(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if matches!(self.select, Select::ReleaseDue(_)) {
            out.push(Action::GbaUp(Btn::Select));
        }
        if !matches!(self.select, Select::Held { .. }) {
            self.select = Select::Held {
                since: now,
                chorded: false,
                sent: None,
            };
        }
        out
    }

    fn hand_over_select(&mut self, now: Millis) -> Vec<Action> {
        match &mut self.select {
            Select::Held {
                chorded: false,
                sent: sent @ None,
                ..
            } => {
                *sent = Some(now);
                vec![Action::GbaDown(Btn::Select)]
            }
            _ => Vec::new(),
        }
    }

    fn select_up(&mut self, now: Millis) -> Vec<Action> {
        let Select::Held { chorded, sent, .. } = std::mem::take(&mut self.select) else {
            return Vec::new();
        };
        match (sent, chorded) {
            (Some(at), _) if now.saturating_sub(at) >= SELECT_TAP_MS => {
                vec![Action::GbaUp(Btn::Select)]
            }
            (Some(at), _) => {
                self.select = Select::ReleaseDue(at + SELECT_TAP_MS);
                Vec::new()
            }
            (None, true) => Vec::new(),
            (None, false) => {
                self.select = Select::ReleaseDue(now + SELECT_TAP_MS);
                let mut out = vec![Action::GbaDown(Btn::Select)];
                if self.context == Context::Shelf {
                    out.push(Action::SelectTap);
                }
                out
            }
        }
    }

    fn menu_down(&mut self, now: Millis) -> Vec<Action> {
        self.menu_chorded = false;
        if self.chording(now) {
            self.mark_chorded();
            self.menu_down_at = None;
            self.menu_last_tap = None;
            return vec![Action::GameMenu];
        }
        if let Some(tap) = self.menu_last_tap {
            if now.saturating_sub(tap) <= MENU_DOUBLE_TAP_MS {
                self.menu_last_tap = None;
                self.menu_down_at = None;
                return vec![Action::Polaroids];
            }
        }
        self.menu_down_at = Some(now);
        self.menu_eject_fired = false;
        Vec::new()
    }

    fn menu_up(&mut self, now: Millis) -> Vec<Action> {
        if self.menu_chorded {
            self.menu_chorded = false;
            self.menu_down_at = None;
            self.menu_eject_fired = false;
            return Vec::new();
        }
        let Some(d) = self.menu_down_at.take() else {
            return Vec::new();
        };
        let ejected = self.menu_eject_fired;
        self.menu_eject_fired = false;
        let tapped = !ejected && now.saturating_sub(d) < MENU_TAP_MS;
        self.menu_last_tap = tapped.then_some(now);
        match tapped {
            true => vec![Action::QuickMenu],
            false => Vec::new(),
        }
    }

    fn power_up(&mut self) -> Vec<Action> {
        let held = self.power_hold_fired;
        self.power_down_at = None;
        self.power_hold_fired = false;
        vec![if held {
            Action::PowerOff
        } else {
            Action::PowerTap
        }]
    }

    fn volume_press(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        let (mine, other, action) = match b {
            Btn::VolUp => (&mut self.vol_up_at, self.vol_down_at, Action::VolumeUp),
            _ => (&mut self.vol_down_at, self.vol_up_at, Action::VolumeDown),
        };
        *mine = Some(now);
        match b {
            Btn::VolUp => self.vol_up_ramp = None,
            _ => self.vol_down_ramp = None,
        }
        let mut out = vec![action];
        let paired = other.is_some_and(|t| now.abs_diff(t) <= MUTE_CHORD_MS);
        if paired && !self.mute_fired {
            self.mute_fired = true;
            out.push(Action::MuteToggle);
        }
        out
    }

    fn volume_release(&mut self, b: Btn) -> Vec<Action> {
        match b {
            Btn::VolUp => (self.vol_up_at, self.vol_up_ramp) = (None, None),
            _ => (self.vol_down_at, self.vol_down_ramp) = (None, None),
        }
        if self.vol_up_at.is_none() && self.vol_down_at.is_none() {
            self.mute_fired = false;
        }
        Vec::new()
    }

    fn rewind_start(&mut self) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        let mut out = Vec::new();
        out.extend(self.ff_clear());
        self.rewinding = true;
        out.push(Action::RewindStart);
        out
    }

    fn rewind_stop(&mut self) -> Vec<Action> {
        if !self.rewinding {
            return Vec::new();
        }
        self.rewinding = false;
        vec![Action::RewindStop]
    }

    fn ff_down(&mut self, now: Millis) -> Vec<Action> {
        if self.rewinding {
            self.r2_refused = true;
            return Vec::new();
        }
        if self.ff_latched {
            self.ff_latching_press = false;
        } else {
            let double = self
                .r2_last_release
                .is_some_and(|rel| now.saturating_sub(rel) <= FF_DOUBLE_TAP_MS);
            self.ff_latched = double;
            self.ff_latching_press = double;
        }
        if self.ff_on {
            return Vec::new();
        }
        self.ff_on = true;
        vec![Action::FfStart]
    }

    fn ff_toggle(&mut self) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        if self.ff_on {
            self.r2_refused = self.ff_button.is_some_and(|f| self.pressed.contains(&f));
            return self.ff_clear();
        }
        self.ff_on = true;
        self.ff_latched = true;
        self.ff_latching_press = false;
        vec![Action::FfStart]
    }

    fn ff_up(&mut self, now: Millis) -> Vec<Action> {
        if std::mem::take(&mut self.r2_refused) {
            return Vec::new();
        }
        self.r2_last_release = Some(now);
        if self.ff_latching_press {
            self.ff_latching_press = false;
            return Vec::new();
        }
        self.ff_clear()
    }

    fn ff_clear(&mut self) -> Vec<Action> {
        self.ff_latched = false;
        self.ff_latching_press = false;
        if !self.ff_on {
            return Vec::new();
        }
        self.ff_on = false;
        vec![Action::FfStop]
    }
}
