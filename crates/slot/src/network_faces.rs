use std::collections::HashMap;

use slot_gfx::{Compositor, TexId};
use slot_ui::{
    footer_ink, keyboard_cap_faces, keyboard_faces, keyboard_text_face, menu_detail_face,
    menu_face, net_bars_face, net_lock_face, prompt_legends, quick_value_face, toast_face_text,
    pair_face, pair_text_width, Keyboard, NetFace, NetIcons, NetLine, Toast, UndoFace,
};

use crate::app::{App, NetLegends, PairFaces, PairView};
use crate::networks::{Line, SCANNING_MESSAGE, WIFI_OFF_MESSAGE};

#[derive(PartialEq)]
struct KeyShape {
    title: String,
    subtitle: Option<String>,
    text: String,
    shift: bool,
    symbols: bool,
}

impl KeyShape {
    fn of(kb: &Keyboard) -> KeyShape {
        KeyShape {
            title: kb.title().to_string(),
            subtitle: kb.subtitle().map(str::to_string),
            text: kb.text(),
            shift: kb.shift(),
            symbols: kb.symbols(),
        }
    }
}

#[derive(Default)]
pub struct NetworkSync {
    texts: HashMap<String, [NetFace; 2]>,
    spare: Vec<TexId>,
    shown: Option<(Vec<Line>, Option<&'static str>)>,
    messages: Vec<(&'static str, NetFace)>,
    statics: bool,
    forget: Vec<(TexId, u32, u32)>,
    forget_for: Option<String>,
    pool: Vec<TexId>,
    keys: Option<KeyShape>,
    forgotten: Option<String>,
    pair: Option<PairView>,
    pair_pool: Vec<TexId>,
}

fn reuse(compositor: &mut Compositor, spare: &mut Vec<TexId>, face: &UndoFace) -> NetFace {
    let id = match spare.pop() {
        Some(id) => {
            compositor.update_texture(id, face.w, face.h, &face.rgba);
            id
        }
        None => compositor.create_texture(face.w, face.h, &face.rgba),
    };
    (id, face.w, face.h)
}

fn swap(compositor: &mut Compositor, at: &mut (TexId, u32, u32), face: &UndoFace) {
    compositor.update_texture(at.0, face.w, face.h, &face.rgba);
    *at = (at.0, face.w, face.h);
}

impl NetworkSync {
    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        self.sync_keyboard(app, compositor);
        self.sync_forgotten(app, compositor);
        self.sync_pair(app, compositor);
        let Some(screen) = app.network_screen() else {
            return;
        };
        if !self.statics {
            self.statics = true;
            self.load_statics(app, compositor);
        }
        let prompt = app.forget_prompt();
        if prompt.is_some() && prompt != self.forget_for {
            if let (Some(title), Some(slot)) = (&prompt, self.forget.first_mut()) {
                swap(compositor, slot, &menu_face(title));
                app.set_forget_faces(self.forget.clone());
            }
            self.forget_for = prompt;
        }
        let key = (screen.lines, screen.message);
        if self.shown.as_ref() == Some(&key) {
            return;
        }
        let wanted: Vec<&str> = key
            .0
            .iter()
            .flat_map(|l| std::iter::once(l.label.as_str()).chain(l.value.as_deref()))
            .collect();
        let stale: Vec<String> = self
            .texts
            .keys()
            .filter(|t| !wanted.contains(&t.as_str()))
            .cloned()
            .collect();
        for t in stale {
            if let Some(pair) = self.texts.remove(&t) {
                self.spare.extend(pair.map(|f| f.0));
            }
        }
        for t in wanted {
            if !self.texts.contains_key(t) {
                let pair = [false, true]
                    .map(|lit| reuse(compositor, &mut self.spare, &quick_value_face(t, lit)));
                self.texts.insert(t.to_string(), pair);
            }
        }
        let lines = key
            .0
            .iter()
            .map(|l| NetLine {
                label: self.texts[&l.label],
                value: l.value.as_ref().map(|v| self.texts[v]),
                lock: l.lock,
                bars: l.bars,
            })
            .collect();
        let message = self
            .messages
            .iter()
            .find(|(m, _)| Some(*m) == key.1)
            .map(|(_, f)| *f);
        app.set_network_faces(lines, message);
        self.shown = Some(key);
    }

    fn load_statics(&mut self, app: &mut App, compositor: &mut Compositor) {
        let mut up = |f: &UndoFace| (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h);
        self.messages = [WIFI_OFF_MESSAGE, SCANNING_MESSAGE]
            .map(|m| (m, up(&menu_detail_face(m))))
            .to_vec();
        app.set_network_icons(NetIcons {
            bars: [0, 1, 2, 3].map(|n| [false, true].map(|l| up(&net_bars_face(n, l)))),
            lock: [false, true].map(|l| up(&net_lock_face(l))),
        });
        let cancel = [false, true].map(|light| {
            let [face, _] = prompt_legends(footer_ink(light), "Cancel", "Join");
            let (id, w, _) = up(&face);
            (id, w)
        });
        let retry = [false, true].map(|light| {
            let [_, face] = prompt_legends(footer_ink(light), "Cancel", "Retry");
            let (id, w, _) = up(&face);
            (id, w)
        });
        let mut word = |w: &str| {
            [false, true].map(|light| {
                let [_, face] = prompt_legends(footer_ink(light), "Back", w);
                let (id, w, _) = up(&face);
                (id, w)
            })
        };
        app.set_network_legends(NetLegends {
            cancel,
            retry,
            scan: word("Scan"),
            join: word("Join"),
            connect: word("Connect"),
            disconnect: word("Disconnect"),
            forget: word("Forget"),
        });
        let [no, yes] = prompt_legends(footer_ink(false), "Cancel", "Forget");
        self.forget = [menu_face("Forget"), no, yes].iter().map(&mut up).collect();
        app.set_forget_faces(self.forget.clone());
    }

    fn sync_pair(&mut self, app: &mut App, compositor: &mut Compositor) {
        let view = app.pair_view();
        if view == self.pair {
            return;
        }
        let Some(now) = view else {
            self.pair = None;
            app.set_pair_faces(None);
            return;
        };
        if !self.statics {
            self.statics = true;
            self.load_statics(app, compositor);
        }
        let mut at = 0;
        let pool = &mut self.pair_pool;
        let mut up = |face: &UndoFace| {
            let id = match pool.get(at) {
                Some(&id) => {
                    compositor.update_texture(id, face.w, face.h, &face.rgba);
                    id
                }
                None => {
                    let id = compositor.create_texture(face.w, face.h, &face.rgba);
                    pool.push(id);
                    id
                }
            };
            at += 1;
            (id, face.w, face.h)
        };
        let qr = now
            .url
            .as_deref()
            .and_then(|url| crate::romm::qr::qr_rgba(url, slot_ui::PAIR_QR))
            .map(|(rgba, side)| up(&UndoFace { rgba, w: side, h: side }));
        let max_w = pair_text_width(app.canvas());
        let lines = now
            .lines
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(slot, text)| (*slot, up(&pair_face(text, *slot, max_w))))
            .collect();
        app.set_pair_faces(Some(PairFaces { qr, lines }));
        self.pair = Some(now);
    }

    fn sync_forgotten(&mut self, app: &App, compositor: &mut Compositor) {
        let name = app.forgotten_network();
        if name == self.forgotten.as_deref() {
            return;
        }
        self.forgotten = name.map(str::to_string);
        let (Some(name), Some(tex)) = (name, app.toast_tex(Toast::Forgotten)) else {
            return;
        };
        let face = toast_face_text(Toast::Forgotten, &format!("{name} was forgotten"));
        compositor.update_texture(tex, face.w, face.h, &face.rgba);
    }

    fn sync_keyboard(&mut self, app: &mut App, compositor: &mut Compositor) {
        let Some(kb) = app.keyboard().cloned() else {
            if self.keys.take().is_some() {
                app.set_keyboard_tex(None);
                for id in &self.pool {
                    compositor.update_texture(*id, 1, 1, &[0; 4]);
                }
            }
            return;
        };
        let now = KeyShape::of(&kb);
        let held = app.take_keyboard_tex();
        let (Some(was), Some(mut tex)) = (self.keys.as_ref(), held) else {
            return self.upload_keyboard(app, compositor, &kb, now);
        };
        if was.title != now.title || was.subtitle != now.subtitle {
            return self.upload_keyboard(app, compositor, &kb, now);
        }
        if was.text != now.text {
            let (face, stops) = keyboard_text_face(&kb);
            swap(compositor, &mut tex.text, &face);
            tex.stops = stops;
        }
        if (was.shift, was.symbols) != (now.shift, now.symbols) {
            let faces = keyboard_cap_faces(&kb, app.canvas());
            for (row, faces) in tex.caps.iter_mut().zip(faces) {
                for (pair, faces) in row.iter_mut().zip(faces) {
                    for (slot, face) in pair.iter_mut().zip(faces.iter()) {
                        swap(compositor, slot, face);
                    }
                }
            }
        }
        app.set_keyboard_tex(Some(tex));
        self.keys = Some(now);
    }

    fn upload_keyboard(
        &mut self,
        app: &mut App,
        compositor: &mut Compositor,
        kb: &Keyboard,
        shape: KeyShape,
    ) {
        let pool = &mut self.pool;
        let mut at = 0;
        let canvas = app.canvas();
        let tex = keyboard_faces(kb, canvas).upload(|f| {
            let id = match pool.get(at) {
                Some(&id) => {
                    compositor.update_texture(id, f.w, f.h, &f.rgba);
                    id
                }
                None => {
                    let id = compositor.create_texture(f.w, f.h, &f.rgba);
                    pool.push(id);
                    id
                }
            };
            at += 1;
            id
        });
        app.set_keyboard_tex(Some(tex));
        self.keys = Some(shape);
    }
}
