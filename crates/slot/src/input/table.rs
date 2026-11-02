use slot_input::{anywhere, Action, Contexts, Entry};
use slot_store::{Binding, CarouselAction, CarouselKeys, Controls, Modifier, ShortcutKeys};
use slot_ui::QuickRow;

use super::bind::{chord, shortcut_action};

#[derive(Copy, Clone, Debug)]
struct Row {
    row: QuickRow,
    binding: Binding,
    action: Option<Action>,
    contexts: Contexts,
}

pub struct Table {
    rows: Vec<Row>,
}

impl Table {
    pub fn new(
        controls: &Controls,
        shortcuts: &ShortcutKeys,
        carousel: &CarouselKeys,
        turbo: bool,
    ) -> Table {
        let rows = QuickRow::ALL
            .into_iter()
            .filter_map(|row| {
                if let Some(g) = row.game_button() {
                    let turbo_row = matches!(row, QuickRow::GameTurboA | QuickRow::GameTurboB);
                    if turbo_row && !turbo {
                        return None;
                    }
                    return controls.game(g).map(|binding| Row {
                        row,
                        binding,
                        action: None,
                        contexts: Contexts::GAME,
                    });
                }
                if let Some(s) = row.shortcut() {
                    let action = shortcut_action(s);
                    return shortcuts.get(s).map(|binding| Row {
                        row,
                        binding,
                        action: Some(action),
                        contexts: match (anywhere(action), binding.modifier) {
                            (false, _) => Contexts::GAME,
                            (true, None) => Contexts::GAME.with(Contexts::SHELF),
                            (true, Some(_)) => Contexts::ALL,
                        },
                    });
                }
                let a = row.carousel_action()?;
                carousel.get(a).map(|binding| Row {
                    row,
                    binding,
                    action: Some(shelf_action(a)),
                    contexts: Contexts::SHELF,
                })
            })
            .collect();
        Table { rows }
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.rows
            .iter()
            .filter_map(|r| {
                r.action.map(|action| Entry {
                    chord: chord(r.binding),
                    action,
                    contexts: r.contexts,
                })
            })
            .collect()
    }

    pub fn clash(&self, row: QuickRow) -> Option<QuickRow> {
        let mine = self.rows.iter().find(|r| r.row == row)?;
        let console = |r: &Row| r.action.is_none();
        self.rows
            .iter()
            .filter(|r| r.row != row && r.contexts.meets(mine.contexts))
            .find(|r| {
                r.binding == mine.binding
                    || (!console(mine)
                        && !console(r)
                        && (overlaps(mine.binding, r.binding) || overlaps(r.binding, mine.binding)))
            })
            .map(|r| r.row)
    }
}

fn shelf_action(a: CarouselAction) -> Action {
    match a {
        CarouselAction::PreviousShelf => Action::PreviousShelf,
        CarouselAction::NextShelf => Action::NextShelf,
        CarouselAction::Favorite => Action::Favorite,
        CarouselAction::ForgetRecent => Action::ForgetRecent,
    }
}

fn overlaps(pair: Binding, single: Binding) -> bool {
    let Some(Modifier::Button(first)) = pair.modifier else {
        return false;
    };
    single.modifier.is_none()
        && (single.button == first || (!first.leads() && single.button == pair.button))
}
