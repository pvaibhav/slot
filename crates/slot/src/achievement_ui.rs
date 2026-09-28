//! Earned banners and a passive shelf sync icon. Rasterization is off the UI thread.
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use slot_achievements::{Notice, NoticeKind, SyncStatus};
use slot_gfx::{Compositor, Draw, TexId, OUT_H, OUT_W};
use slot_ui::{text, CartFace, SyncIndicator};

use crate::app::Phase;
use crate::session::Session;

const WIDTH: u32 = 360;
const HEIGHT: u32 = 44;
const DURATION: Duration = Duration::from_millis(1800);
const BADGE_PX: u32 = 32;

pub(crate) struct Notifications {
    requests: Sender<Request>,
    built: Receiver<Built>,
    waiting: bool,
    texture: Option<TexId>,
    badge: Option<TexId>,
    shown: Option<Instant>,
    sync: Option<TexId>,
    started: Instant,
    requested_progress: Option<u8>,
    displayed_progress: Option<u8>,
    progress: Option<TexId>,
}

enum Request {
    Earned(Notice, u32),
    Progress(u8),
}

enum Built {
    Progress(u8, CartFace),
    Sync(CartFace),
    Banner(CartFace, CartFace, Vec<i16>),
}

impl Notifications {
    pub fn new() -> Self {
        let (requests, inbox) = mpsc::channel();
        let (outbox, built) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("slot-ra-banner".into())
            .spawn(move || {
                if outbox.send(Built::Sync(slot_ui::sync_icon_face())).is_err() {
                    return;
                }
                while let Ok(request) = inbox.recv() {
                    let (notice, rate) = match request {
                        Request::Earned(notice, rate) => (notice, rate),
                        Request::Progress(percent) => {
                            if outbox
                                .send(Built::Progress(percent, progress_face(percent)))
                                .is_err()
                            {
                                return;
                            }
                            continue;
                        }
                    };
                    let sound = if rate == 0 {
                        Vec::new()
                    } else {
                        crate::audio::Sfx::Achievement.render(rate)
                    };
                    if outbox
                        .send(Built::Banner(face(&notice), badge_face(&notice), sound))
                        .is_err()
                    {
                        return;
                    }
                }
            });
        Self {
            requests,
            built,
            waiting: false,
            texture: None,
            badge: None,
            shown: None,
            sync: None,
            started: Instant::now(),
            requested_progress: None,
            displayed_progress: None,
            progress: None,
        }
    }

    pub fn update(&mut self, session: &mut Session, compositor: &mut Compositor) {
        if matches!(session.app().phase(), Phase::Doze { .. }) || session.app().shutting_down() {
            return;
        }
        let percent = session.achievement_sync_progress();
        if percent != self.requested_progress {
            self.requested_progress = percent;
            if let Some(percent) = percent {
                let _ = self.requests.send(Request::Progress(percent));
            }
        }
        while let Ok(built) = self.built.try_recv() {
            match built {
                Built::Progress(percent, face) => {
                    if self.requested_progress != Some(percent) {
                        continue;
                    }
                    match self.progress {
                        Some(id) => compositor.update_texture(id, face.w, face.h, &face.rgba),
                        None => {
                            self.progress =
                                Some(compositor.create_texture(face.w, face.h, &face.rgba))
                        }
                    }
                    self.displayed_progress = Some(percent);
                }
                Built::Sync(face) => {
                    self.sync = Some(compositor.create_texture(face.w, face.h, &face.rgba));
                }
                Built::Banner(face, badge, sound) => {
                    self.waiting = false;
                    match self.texture {
                        Some(id) => compositor.update_texture(id, face.w, face.h, &face.rgba),
                        None => {
                            self.texture =
                                Some(compositor.create_texture(face.w, face.h, &face.rgba))
                        }
                    }
                    match self.badge {
                        Some(id) => compositor.update_texture(id, badge.w, badge.h, &badge.rgba),
                        None => {
                            self.badge =
                                Some(compositor.create_texture(badge.w, badge.h, &badge.rgba))
                        }
                    }
                    self.shown = Some(Instant::now());
                    session.mix_sfx(sound);
                }
            }
        }
        let status = session.achievement_sync_status();
        let visible = status == SyncStatus::Syncing || session.achievement_sync_pending();
        let icon = self.sync.and_then(|face| {
            visible.then_some(SyncIndicator {
                face,
                turn: if status == SyncStatus::Syncing {
                    self.started.elapsed().as_secs_f32() % 2.0 * std::f32::consts::PI
                } else {
                    0.0
                },
                alpha: match status {
                    SyncStatus::Syncing => 0.9,
                    _ => 0.35,
                },
                attention: matches!(status, SyncStatus::Offline | SyncStatus::Attention),
                progress: if percent.is_some() && self.displayed_progress.is_some() {
                    self.progress
                } else {
                    None
                },
            })
        });
        session.app_mut().set_achievement_sync(icon);
    }

    pub fn draw(&mut self, session: &Session, draws: &mut Vec<Draw>) {
        if matches!(session.app().phase(), Phase::Doze { .. }) || session.app().shutting_down() {
            return;
        }
        if self.shown.is_some_and(|time| time.elapsed() >= DURATION) {
            self.shown = None;
        }
        if !self.waiting && self.shown.is_none() {
            while let Some(notice) = session.take_achievement_notice() {
                if notice.kind == NoticeKind::Earned {
                    self.waiting = self
                        .requests
                        .send(Request::Earned(notice, session.audio_ring().sample_rate()))
                        .is_ok();
                    break;
                }
            }
        }
        if let (Some(tex), Some(badge), Some(shown)) = (self.texture, self.badge, self.shown) {
            draw_banner(tex, badge, shown.elapsed().as_secs_f32(), draws);
        }
    }
}

fn progress_face(percent: u8) -> CartFace {
    let mut rgba = vec![0; (36 * slot_ui::HINT_H * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, &format!("{percent}%"), 36.0, 1, 12.0, 12.0);
        text::draw_centred(&mut rgba, 36, slot_ui::HINT_H, &layout, slot_ui::HUD_INK);
    }
    CartFace {
        rgba,
        w: 36,
        h: slot_ui::HINT_H,
    }
}

fn draw_banner(tex: TexId, badge: TexId, elapsed: f32, draws: &mut Vec<Draw>) {
    if elapsed >= DURATION.as_secs_f32() {
        return;
    }
    let enter = (elapsed / 0.20).clamp(0.0, 1.0);
    let exit = ((elapsed - 1.55) / 0.25).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - enter).powi(3);
    let alpha = ease * (1.0 - exit * exit);
    let x = (OUT_W - WIDTH) as f32 / 2.0;
    let y = (OUT_H - HEIGHT - 12) as f32 + (1.0 - ease) * 12.0 + exit * exit * 6.0;
    draws.push(Draw::Tex {
        x,
        y,
        w: WIDTH as f32,
        h: HEIGHT as f32,
        tex,
        alpha,
    });
    // One restrained overshoot: the badge settles while the card finishes sliding in.
    let pop = (elapsed / 0.32).clamp(0.0, 1.0);
    let scale = 1.0 + 0.12 * (std::f32::consts::PI * pop).sin();
    let size = BADGE_PX as f32 * scale;
    draws.push(Draw::Tex {
        x: x + 8.0 + (BADGE_PX as f32 - size) / 2.0,
        y: y + (HEIGHT as f32 - size) / 2.0,
        w: size,
        h: size,
        tex: badge,
        alpha,
    });
}

fn badge_face(notice: &Notice) -> CartFace {
    let Some(image) = notice
        .badge
        .as_deref()
        .and_then(slot_achievements::load_badge)
    else {
        return slot_ui::achievement_icon_face(BADGE_PX);
    };
    let mut rgba = vec![0; (BADGE_PX * BADGE_PX * 4) as usize];
    for y in 0..BADGE_PX {
        for x in 0..BADGE_PX {
            let src = (((y * image.height / BADGE_PX) * image.width + x * image.width / BADGE_PX)
                * 4) as usize;
            let dst = ((y * BADGE_PX + x) * 4) as usize;
            rgba[dst..dst + 4].copy_from_slice(&image.rgba[src..src + 4]);
        }
    }
    CartFace {
        rgba,
        w: BADGE_PX,
        h: BADGE_PX,
    }
}

fn face(notice: &Notice) -> CartFace {
    let mut rgba = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[18, 18, 18, 235]);
    }
    if let Some(font) = text::label_font() {
        const TEXT_X: u32 = 48;
        const TEXT_W: u32 = WIDTH - TEXT_X - 12;
        let mut title_rgba = vec![0; (TEXT_W * HEIGHT * 4) as usize];
        for pixel in title_rgba.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[18, 18, 18, 235]);
        }
        let title: String = notice.title.chars().take(180).collect();
        let layout = text::fit_box(
            font,
            &title,
            TEXT_W as f32,
            (HEIGHT - 8) as f32,
            2,
            16.0,
            12.0,
        );
        text::draw_centred(&mut title_rgba, TEXT_W, HEIGHT, &layout, [245, 245, 245]);
        for y in 0..HEIGHT as usize {
            let dst = (y * WIDTH as usize + TEXT_X as usize) * 4;
            let src = y * TEXT_W as usize * 4;
            rgba[dst..dst + TEXT_W as usize * 4]
                .copy_from_slice(&title_rgba[src..src + TEXT_W as usize * 4]);
        }
    }
    CartFace {
        rgba,
        w: WIDTH,
        h: HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_enters_settles_and_is_gone_before_two_seconds() {
        let tex = TexId::from_raw(0);
        let mut draws = Vec::new();
        draw_banner(tex, tex, 0.1, &mut draws);
        assert!(
            matches!(draws[0], Draw::Tex { alpha, y, .. } if alpha > 0.0 && alpha < 1.0 && y > (OUT_H - HEIGHT - 12) as f32)
        );
        draws.clear();
        draw_banner(tex, tex, 0.5, &mut draws);
        assert!(matches!(draws[0], Draw::Tex { alpha: 1.0, .. }));
        assert!(matches!(draws[1], Draw::Tex { w, .. } if w == BADGE_PX as f32));
        draws.clear();
        draw_banner(tex, tex, 1.7, &mut draws);
        assert!(matches!(draws[0], Draw::Tex { alpha, .. } if alpha > 0.0 && alpha < 1.0));
        draws.clear();
        draw_banner(tex, tex, 1.8, &mut draws);
        assert!(draws.is_empty());
    }

    #[test]
    fn achievement_banner_renders_a_title_in_a_bounded_panel() {
        let notice = Notice {
            badge: std::env::var_os("SLOT_RA_PREVIEW_BADGE").map(std::path::PathBuf::from),
            generation: 1,
            title: "First Steps (+5)".into(),
            detail: String::new(),
            kind: NoticeKind::Earned,
        };
        let banner = face(&notice);
        assert_eq!(banner.rgba.len(), (WIDTH * HEIGHT * 4) as usize);
        assert!(banner.rgba.chunks_exact(4).filter(|p| p[0] > 150).count() > 100);
        if let Some(dir) = std::env::var_os("SCRATCH_PNG_DIR") {
            let file =
                std::fs::File::create(std::path::Path::new(&dir).join("slot-achievement.png"))
                    .unwrap();
            let mut encoder = png::Encoder::new(file, WIDTH, HEIGHT);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&banner.rgba)
                .unwrap();
        }
    }

    /// Optional native UI exports, using a recorded game frame when one is supplied.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "writes UI previews; set SCRATCH_PNG_DIR and optionally SLOT_RA_PREVIEW_FRAME"]
    fn render_achievement_previews() {
        use crate::app::App;
        use slot_gfx::HeadlessSurface;
        use slot_power::{Power, SimPlatform};

        let dir = std::path::PathBuf::from(std::env::var_os("SCRATCH_PNG_DIR").unwrap());
        let surface = HeadlessSurface::new().unwrap();
        let mut compositor = Compositor::new(&surface).unwrap();
        let save = |name: &str, pixels: &[u8]| {
            let file = std::fs::File::create(dir.join(format!("slot-ra-{name}.png"))).unwrap();
            let mut encoder = png::Encoder::new(file, OUT_W, OUT_H);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        };
        let root = tempfile::tempdir().unwrap();
        let carts: Vec<_> = ["Metroid Fusion", "Pokemon Emerald", "Golden Sun"]
            .iter()
            .map(|name| slot_store::Cart {
                platform: slot_store::Platform::Gba,
                stem: (*name).into(),
                title: (*name).into(),
                rom: root.path().join(format!("{name}.gba")),
                label: None,
                code: String::new(),
                shell: None,
            })
            .collect();
        let faces = carts
            .iter()
            .map(|cart| {
                let f = slot_ui::cart_face(cart);
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        let mut app = App::new(carts);
        app.set_faces(faces);
        app.set_power(Power::new(
            Box::new(SimPlatform::at(root.path().into())),
            Duration::from_secs(180),
        ));
        app.update(1.0);
        let clock = slot_ui::word_face("14:32");
        app.set_shelf_clock_face(
            compositor.create_texture(clock.w, clock.h, &clock.rgba),
            clock.w,
        );
        let percent = slot_ui::word_face("100%");
        app.set_battery_percent_face(
            compositor.create_texture(percent.w, percent.h, &percent.rgba),
            percent.w,
        );
        let icon = slot_ui::sync_icon_face();
        let icon = compositor.create_texture(icon.w, icon.h, &icon.rgba);
        let mut draws = Vec::new();
        for (name, turn, alpha, attention) in [
            ("sync", 0.3, 0.9, false),
            ("ready", 0.0, 0.55, false),
            ("waiting", 0.0, 0.35, true),
        ] {
            app.set_achievement_sync(Some(SyncIndicator {
                face: icon,
                turn,
                alpha,
                attention,
                progress: if name == "sync" {
                    let f = progress_face(62);
                    Some(compositor.create_texture(f.w, f.h, &f.rgba))
                } else {
                    None
                },
            }));
            draws.clear();
            app.draw(&mut draws);
            compositor.begin_frame();
            compositor.draw_list(&draws);
            save(name, &compositor.read_frame());
        }
        let notice = Notice {
            badge: std::env::var_os("SLOT_RA_PREVIEW_BADGE").map(std::path::PathBuf::from),
            generation: 1,
            title: "First Steps (+5)".into(),
            detail: String::new(),
            kind: NoticeKind::Earned,
        };
        let banner = face(&notice);
        let mut notifications = Notifications::new();
        notifications.texture = Some(compositor.create_texture(banner.w, banner.h, &banner.rgba));
        let badge = badge_face(&notice);
        notifications.badge = Some(compositor.create_texture(badge.w, badge.h, &badge.rgba));
        notifications.shown = Some(Instant::now() - Duration::from_millis(500));
        draws.clear();
        if let Some(path) = std::env::var_os("SLOT_RA_PREVIEW_FRAME") {
            let rgba = slot_ui::wallpaper_face(std::path::Path::new(&path)).unwrap();
            let tex = compositor.create_texture(OUT_W, OUT_H, &rgba);
            draws.push(Draw::Tex {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                tex,
                alpha: 1.0,
            });
        }
        notifications.draw(&Session::boot(root.path().into()), &mut draws);
        compositor.begin_frame();
        compositor.draw_list(&draws);
        save("earned", &compositor.read_frame());
        if std::env::var_os("SLOT_RA_PREVIEW_ANIMATE").is_some() {
            // Capture the actual drawing path at 30 fps, including a clean frame at the end.
            draws.truncate(draws.len() - 2);
            for frame in 0..=60 {
                let mut animated = draws.clone();
                draw_banner(
                    notifications.texture.unwrap(),
                    notifications.badge.unwrap(),
                    frame as f32 / 30.0,
                    &mut animated,
                );
                compositor.begin_frame();
                compositor.draw_list(&animated);
                save(&format!("animation-{frame:03}"), &compositor.read_frame());
            }
        }
    }
}
