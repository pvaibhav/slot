use slot_input::{Action, Btn};
use slot_store::KidMode;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum KidWhere {
    Shelf,
    Playing,
    Other,
}

pub fn allowed(at: KidWhere, mode: KidMode, action: Action) -> Option<Action> {
    let kept = match (at, action) {
        (_, Action::VolumeUp | Action::VolumeDown | Action::MuteToggle) => true,
        (KidWhere::Playing, Action::QuickMenu) => return Some(Action::Eject),
        (KidWhere::Playing, Action::GbaDown(_) | Action::GbaUp(_)) => true,
        (KidWhere::Playing, Action::BrightnessUp | Action::BrightnessDown) => true,
        (KidWhere::Shelf, Action::GbaDown(b) | Action::GbaUp(b)) => {
            matches!(b, Btn::A | Btn::Up | Btn::Down | Btn::Left | Btn::Right)
        }
        (KidWhere::Shelf, Action::ShelfLeft | Action::ShelfRight | Action::Insert) => true,
        (KidWhere::Shelf, Action::BrightnessUp | Action::BrightnessDown) => {
            mode == KidMode::OneShelf
        }
        (
            KidWhere::Shelf,
            Action::PreviousShelf
            | Action::NextShelf
            | Action::PreviousCollection
            | Action::NextCollection,
        ) => mode == KidMode::Shelves,
        (KidWhere::Other, Action::GbaDown(b) | Action::GbaUp(b)) => matches!(b, Btn::A | Btn::B),
        _ => false,
    };
    kept.then_some(action)
}

pub fn unlock_chord(held: &[Btn]) -> bool {
    [Btn::Start, Btn::Select, Btn::L2, Btn::R2]
        .iter()
        .all(|b| held.contains(b))
}
