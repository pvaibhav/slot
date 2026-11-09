use slot_gfx::{Compositor, Draw, TexId, DS_SCREEN_H, DS_SCREEN_W};

use crate::frames::FrameRef;
use crate::session::Session;

#[derive(Default)]
pub struct DsView {
    still: Option<TexId>,
    rgba: Vec<u8>,
    last: Vec<u8>,
    top: bool,
}

pub fn playing(session: &Session) -> bool {
    #[cfg(feature = "nds")]
    {
        let app = session.app();
        matches!(app.phase(), crate::app::Phase::Playing { .. })
            && app.playing_platform() == slot_store::Platform::Nds
            && session.emu().is_some()
    }
    #[cfg(not(feature = "nds"))]
    {
        let _ = session;
        false
    }
}

pub fn seated(session: &Session) -> bool {
    #[cfg(feature = "nds")]
    {
        session
            .app()
            .seated_cart()
            .is_some_and(|c| c.platform == slot_store::Platform::Nds)
    }
    #[cfg(not(feature = "nds"))]
    {
        let _ = session;
        false
    }
}

fn frame(session: &Session) -> Option<FrameRef> {
    let frame = session.frame().filter(|_| playing(session))?;
    (frame.len() >= (DS_SCREEN_W * DS_SCREEN_H * 2 * 4) as usize).then_some(frame)
}

pub fn direct(session: &Session) -> bool {
    let app = session.app();
    playing(session) && !app.game_menu_open() && app.in_game_menu().is_none()
}

impl DsView {
    pub fn swap_game(
        &mut self,
        draws: &mut [Draw],
        session: &Session,
        compositor: &mut Compositor,
        screen: crate::screen::Screen,
    ) {
        if !seated(session) {
            self.last.clear();
            return;
        }
        let top = screen == crate::screen::Screen::Top;
        if let Some(frame) = frame(session) {
            self.last.clear();
            self.last.extend_from_slice(&frame);
            self.top = top;
            self.still = Some(self.take(top, self.still, compositor));
        } else if top != self.top && !self.last.is_empty() {
            self.top = top;
            self.still = Some(self.take(top, self.still, compositor));
        }
        let Some(tex) = self.still else {
            return;
        };
        for d in draws.iter_mut() {
            if matches!(d, Draw::Game) {
                *d = Draw::Shot { tex };
            }
        }
    }

    pub fn showing_top(&self) -> bool {
        self.top
    }

    pub fn last_frame(&self) -> Option<&[u8]> {
        (!self.last.is_empty()).then_some(&self.last[..])
    }

    fn take(&mut self, top: bool, into: Option<TexId>, compositor: &mut Compositor) -> TexId {
        let n = (DS_SCREEN_W * DS_SCREEN_H) as usize;
        let from = if top { 0 } else { n * 4 };
        self.rgba.clear();
        self.rgba.reserve(n * 4);
        for px in self.last[from..from + n * 4].chunks_exact(4) {
            self.rgba.extend_from_slice(&[px[2], px[1], px[0], 0xff]);
        }
        match into {
            Some(id) => {
                compositor.update_texture(id, DS_SCREEN_W, DS_SCREEN_H, &self.rgba);
                id
            }
            None => compositor.create_texture_nearest(DS_SCREEN_W, DS_SCREEN_H, &self.rgba),
        }
    }
}

pub fn set_touch(session: &Session, at: Option<(u8, u8)>) {
    if let Some(emu) = session.emu().filter(|_| playing(session)) {
        emu.set_touch(at);
    }
}
