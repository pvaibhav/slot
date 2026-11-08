use std::time::{Duration, Instant};

use slot_gfx::{Compositor, Draw, TexId, OUT_H, OUT_W};

const PAPER: &[u8] = include_bytes!("../assets/paper.png");
use crate::screen::Screen;
use crate::shot_loader::{ShotLoader, Want};
use slot_input::{InputSource, Millis};
use slot_power::{Platform, Power};
use slot_store::{format_stamp, BezelPick, Platform as Console};
use slot_ui::{
    badge_face, bare_board_face, bezel_face, cart_shadow, chip_face, chip_face_text,
    chip_shadow_face, core_button_legends, date_time_text, default_game_bezel_face, delete_legends,
    footer_ink, game_bezel_face, gb_cart_shadow, hint_face, icon_face, link_button_legends,
    menu_detail_face, menu_face, padded, photo_face, prompt_legends, quick_description_face,
    quick_label_face, quick_legend_faces, quick_pointer_face, quick_value_face,
    quick_warning_value_face, rebuild_count_face, rebuild_title_face, repair_legends, slot_back,
    slot_back_light, slot_front, slot_front_light, socket_face, socket_face_text, sticker_face,
    time_text, title_face, toast_face, update_cart_face, update_legends, wallpaper_face, word_ink,
    BezelArt, CartFace, GbShell, Icon, LinkBadge, PowerChoice, QuickMenuFaces, QuickRow,
    QuickValue, RebuildScreen, StickerFields, Toast, UndoFace, ALERT_PX, BOLT_PX, HUD_ICON_PX,
    HUD_INK, LEGEND, TURN_PAD,
};

use crate::app::{App, LinkRow, Phase, UpdateCart, UpdateScene};

use crate::build_info::Build;
use crate::cart_faces::CartFaces;
use crate::face_builder::FaceBuilder;
use crate::label_cache::{self, Rebuild};
use crate::link_art_builder::LinkArtBuilder;
use crate::link_screen::{LinkSprites, Sprite};
use crate::link_start::{LinkFail, LinkStep};
use crate::session::Session;

const DOZE_TIMEOUT: Duration = Duration::from_secs(180);

const ALERT_INK: [u8; 3] = [0xf0, 0xb4, 0x3c];

pub struct Frontend {
    session: Session,
    grounds: Option<(TexId, Option<TexId>, (u32, u32))>,
    start: Instant,
    last: Instant,
    draws: Vec<Draw>,
    ds: crate::ds_view::DsView,
    polaroid_texes: Vec<TexId>,
    title_tex: Option<TexId>,
    faces: FaceBuilder,
    cart_faces: CartFaces,
    rebuild: Option<Rebuild>,
    stale_check: Option<std::sync::mpsc::Receiver<Vec<slot_store::Cart>>>,
    ui_faces: Option<std::sync::mpsc::Receiver<UiFaces>>,
    rebuild_faces: RebuildFaces,
    link_art: LinkArtBuilder,
    link_art_done: bool,
    core_asked: Option<(Console, String)>,
    core_tex: CoreTextures,
    core_built: Option<(Console, String)>,
    undo_tex: Option<TexId>,
    switcher: Switcher,
    guide_page: GuidePage,
    clocks: Clocks,
    box_text: BoxTextSync,
    box_art: BoxArtSync,
    about: AboutFace,
    update_text: UpdateText,
    fetch_sync: FetchSync,
    studio_sync: StudioSync,
    print_tex: Option<crate::print_scene::PrintTextures>,
    gift_boxes: crate::unwrap_scene::GiftBoxes,
    print_stage: Option<std::sync::mpsc::Receiver<Option<crate::print_scene::StageArt>>>,
    print_stage_asked: bool,
    quick_clock: QuickClock,
    quick_network: QuickClock,
    quick_romm: [QuickClock; 4],
    quick_kid_shelf: QuickClock,
    networks: crate::network_faces::NetworkSync,
    quick_palette: QuickPalette,
    wallpaper: WallpaperSync,
    picking_faces: PickingSync,
    game_bezel: BezelSync,
    overlay: OverlaySync,
    look: Option<ShelfLook>,
    link_shot: Option<TexId>,
    link_shot_size: (u32, u32),
    link_shot_shown: bool,
}

#[derive(Default)]
struct RebuildFaces {
    title: Option<(TexId, u32, u32)>,
    count: Option<TexId>,
    count_size: (u32, u32),
    shown: Option<usize>,
}

#[derive(Default)]
struct QuickClock {
    dim: Option<TexId>,
    lit: Option<TexId>,
    shown: String,
}

#[derive(Default)]
struct QuickPalette {
    dim: Option<TexId>,
    lit: Option<TexId>,
    shown: &'static str,
}

type BezelImage = (Console, BezelPick, Option<BezelArt>, (u32, u32));

#[derive(Default)]
struct BezelSync {
    tex: Option<TexId>,
    image: Option<BezelImage>,
    labels: [String; 3],
    faces: [(Option<TexId>, Option<TexId>); 3],
}

#[derive(Default)]
struct OverlaySync {
    want: Option<(Console, Option<slot_store::OverlayEntry>)>,
    serial: u64,
    loader: Option<crate::overlay::OverlayLoader>,
    tile: Option<TexId>,
    full: Option<TexId>,
    full_bottom: Option<TexId>,
    shown: Option<ShownOverlay>,
    ds: Option<crate::overlay::DecodedOverlay>,
    labels: [String; 4],
    faces: [(Option<TexId>, Option<TexId>); 4],
}

#[derive(Clone, Copy)]
struct ShownOverlay {
    tile: Option<(TexId, u32, u32)>,
    top: Option<(TexId, u32, u32)>,
    bottom: Option<(TexId, u32, u32)>,
}

#[derive(Default)]
struct WallpaperSync {
    tex: Option<TexId>,
    image: Option<(String, (u32, u32))>,
    label: String,
    dim: Option<TexId>,
    lit: Option<TexId>,
    loader: Option<crate::wallpaper::WallpaperLoader>,
    shelf_faces: std::collections::HashMap<(String, (u32, u32)), TexId>,
    shelf_asked: std::collections::HashSet<(String, (u32, u32))>,
    shelf_primed: Option<(u32, u32)>,
}

struct StudioSync {
    worker: crate::studio_faces::StudioWorker,
    setup: bool,
    session: u64,
    chips_for: Option<[u8; 3]>,
    gen: u64,
    last: Option<crate::studio_faces::PreviewJob>,
    face_slots: [Option<TexId>; 4],
    bare: Option<TexId>,
    under: [Option<TexId>; 2],
    chips: Option<TexId>,
}

impl StudioSync {
    fn new() -> StudioSync {
        StudioSync {
            worker: crate::studio_faces::StudioWorker::spawn(),
            setup: false,
            session: 0,
            chips_for: None,
            gen: 0,
            last: None,
            face_slots: [None; 4],
            bare: None,
            under: [None; 2],
            chips: None,
        }
    }
}

fn sync_studio(app: &mut App, compositor: &mut Compositor, st: &mut StudioSync) {
    use crate::studio::{LabelKind, Legend};
    use crate::studio_faces::StudioDone;
    use crate::studio_view::Style;
    use slot_ui::StudioGlyph as G;
    if !st.setup {
        st.setup = true;
        for (legend, glyph, word) in [
            (Legend::Back, G::B, "Back"),
            (Legend::Open, G::A, "Open"),
            (Legend::Apply, G::A, "Apply"),
            (Legend::Change, G::LeftRight, "Change"),
            (Legend::ChangeVertical, G::UpDown, "Change"),
            (Legend::Move, G::LeftRight, "Move"),
            (Legend::Reset, G::X, "Reset"),
            (Legend::Hex, G::Y, "Hex"),
        ] {
            for (left, light) in [(true, false), (false, false), (true, true), (false, true)] {
                let face = slot_ui::studio_legend(glyph, word, footer_ink(light), left);
                let tex = compositor.create_texture(face.w, face.h, &face.rgba);
                app.set_studio_legend((legend, left, light), (tex, face.w));
            }
        }
        let liner = slot_ui::liner::liner_face(136, 121);
        let liner = compositor.create_texture(liner.w, liner.h, &liner.rgba);
        let mut disc = vec![255u8; 64 * 64 * 4];
        for y in 0..64 {
            for x in 0..64 {
                let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
                disc[(y * 64 + x) * 4 + 3] =
                    ((32.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
        let disc = compositor.create_texture(64, 64, &disc);
        app.set_studio_paper(liner, disc);
    }
    if app.studio().is_none() {
        st.last = None;
        return;
    }
    let chips_for = app.studio().and_then(|s| s.opened_ground);
    if st.session != app.studio_session() || st.chips_for != chips_for {
        st.session = app.studio_session();
        st.chips_for = chips_for;
        if let Some(studio) = app.studio() {
            st.worker.ask_chips(studio.cart.clone(), chips_for);
        }
    }
    if let Some(job) = app.studio_preview() {
        if st.last.as_ref() != Some(&job) {
            st.last = Some(job.clone());
            st.gen += 1;
            st.worker.ask(crate::studio_faces::PreviewJob { gen: st.gen, ..job });
        }
    }
    for (text, style) in app.studio_wanted_text() {
        let mut face = match style {
            Style::Heading => slot_ui::heading_face(&text),
            Style::Digit | Style::DigitOn => menu_face(&text),
            Style::Detail => menu_detail_face(&text),
        };
        if style == Style::DigitOn {
            for px in face.rgba.chunks_exact_mut(4) {
                px[..3].copy_from_slice(&[0x1c, 0x1a, 0x18]);
            }
        }
        let tex = compositor.create_texture(face.w, face.h, &face.rgba);
        app.set_studio_text((text, style), (tex, face.w, face.h));
    }
    while let Some(done) = st.worker.take() {
        match done {
            StudioDone::Chips(atlas) => {
                let tex = upload_rgba(compositor, &mut st.chips, atlas.w, atlas.h, &atlas.rgba);
                app.set_studio_chips((tex, atlas.w, atlas.h));
            }
            StudioDone::Preview(faces) => {
                if faces.session != st.session {
                    continue;
                }
                let slot_of = |k: LabelKind| match k {
                    LabelKind::Existing => 0,
                    LabelKind::Real => 1,
                    LabelKind::Logo => 2,
                    LabelKind::Title => 3,
                };
                let (w, h) = (faces.bare.w, faces.bare.h);
                let mut texes = Vec::new();
                for (kind, face) in &faces.faces {
                    let slot = &mut st.face_slots[slot_of(*kind)];
                    texes.push((*kind, upload_rgba(compositor, slot, face.w, face.h, &face.rgba)));
                }
                let bare = upload_rgba(compositor, &mut st.bare, w, h, &faces.bare.rgba);
                let under = faces.under.as_ref().map(|[a, b]| {
                    let [ua, ub] = &mut st.under;
                    [
                        upload_rgba(compositor, ua, a.w, a.h, &a.rgba),
                        upload_rgba(compositor, ub, b.w, b.h, &b.rgba),
                    ]
                });
                app.set_studio_tex(crate::app::StudioTex { faces: texes, bare, under, w, h });
            }
        }
    }
}

struct FetchSync {
    faces: crate::fetch_faces::FetchFaces,
    setup: bool,
    examples_asked: bool,
    lines: Vec<String>,
    text: Vec<Option<TexId>>,
    example_slots: [Option<TexId>; 2],
    sets: [Option<(slot_store::Platform, String)>; 2],
    set_slots: [[Option<TexId>; 5]; 2],
    asked: Vec<(slot_store::Platform, String)>,
}

impl FetchSync {
    fn new() -> FetchSync {
        FetchSync {
            faces: crate::fetch_faces::FetchFaces::spawn(),
            setup: false,
            examples_asked: false,
            lines: Vec::new(),
            text: Vec::new(),
            example_slots: [None, None],
            sets: [None, None],
            set_slots: [[None; 5]; 2],
            asked: Vec::new(),
        }
    }
}

fn sync_server(
    app: &mut App,
    compositor: &mut Compositor,
    tex: Option<crate::print_scene::PrintTextures>,
) {
    use crate::print_scene::PrintArtState;
    let Some(have) = tex else {
        return;
    };
    app.set_server_tex(have);
    match app.take_server_art() {
        PrintArtState::Pending => {}
        PrintArtState::Failed => app.server_art_failed(),
        PrintArtState::Ready(art) => {
            have.set_paper(compositor, &art);
            app.server_paper_up();
        }
    }
}

fn sync_fetch_art(app: &mut App, compositor: &mut Compositor, st: &mut FetchSync) {
    use crate::fetch_art::FetchTex;
    use crate::fetch_faces::{FaceDone, FaceJob};
    if !st.setup {
        st.setup = true;
        let mut up = |f: slot_ui::UndoFace| (compositor.create_texture(f.w, f.h, &f.rgba), f.w);
        let mut set = |ink: [u8; 3]| {
            let [cancel, pick] = slot_ui::fetch_choice_legends(ink);
            let stop = slot_ui::fetch_legends(ink);
            let [done, _] = prompt_legends(ink, "Done", "");
            let skip = slot_ui::fetch_skip_legend(ink);
            [up(cancel), up(pick), up(stop), up(done), up(skip)]
        };
        let legends = [set(footer_ink(false)), set(footer_ink(true))];
        app.set_fetch_legends(legends);
        let paper = compositor.create_texture(1, 1, &[0xf6, 0xf4, 0xef, 255]);
        app.set_fetch_paper(paper);
    }
    if !st.examples_asked && app.wifi_configured() {
        st.examples_asked = true;
        st.faces.ask(FaceJob::Examples);
    }
    let open = app.fetch_art().is_some();
    if !open {
        st.asked.clear();
        st.sets = [None, None];
        st.lines.clear();
    }
    let lines = app.fetch_art_lines();
    if open && lines != st.lines {
        st.text.resize(lines.len().max(st.text.len()), None);
        let mut texes = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let face = match i {
                0 => menu_face(line),
                _ => menu_detail_face(line),
            };
            let tex = upload_rgba(compositor, &mut st.text[i], face.w, face.h, &face.rgba);
            texes.push((tex, face.w, face.h));
        }
        app.set_fetch_text(texes);
        st.lines = lines;
    }
    let wanted = app.fetch_wanted();
    let is_wanted = |p: slot_store::Platform, s: &str| {
        wanted.iter().any(|l| l.platform == p && l.stem == s)
    };
    for set in st.sets.iter_mut() {
        if set.as_ref().is_some_and(|(p, s)| !is_wanted(*p, s)) {
            *set = None;
        }
    }
    st.asked.retain(|(p, s)| is_wanted(*p, s));
    for l in &wanted {
        let key = (l.platform, l.stem.clone());
        if st.asked.contains(&key) {
            continue;
        }
        if let Some(mut cart) = app.fetch_cart(l.platform, &l.stem) {
            cart.label = Some(l.path.clone());
            st.faces.ask(FaceJob::Cart(cart));
            st.asked.push(key);
        }
    }
    while let Some(done) = st.faces.take() {
        match done {
            FaceDone::Examples([a, b]) => {
                let ta = upload_rgba(compositor, &mut st.example_slots[0], a.w, a.h, &a.rgba);
                let tb = upload_rgba(compositor, &mut st.example_slots[1], b.w, b.h, &b.rgba);
                app.set_fetch_example_tex([(ta, a.w, a.h), (tb, b.w, b.h)]);
            }
            FaceDone::Cart(platform, stem, set) => {
                if !is_wanted(platform, &stem) {
                    continue;
                }
                let Some(i) = st.sets.iter().position(Option::is_none) else {
                    continue;
                };
                st.sets[i] = Some((platform, stem.clone()));
                let slots = &mut st.set_slots[i];
                let title = upload_rgba(compositor, &mut slots[0], set.title.w, set.title.h, &set.title.rgba);
                let bare = upload_rgba(compositor, &mut slots[1], set.bare.w, set.bare.h, &set.bare.rgba);
                let finished = upload_rgba(compositor, &mut slots[2], set.done.w, set.done.h, &set.done.rgba);
                let under = set.under.as_ref().map(|[interior, backing]| {
                    let (a, b) = slots[3..5].split_at_mut(1);
                    [
                        upload_rgba(compositor, &mut a[0], interior.w, interior.h, &interior.rgba),
                        upload_rgba(compositor, &mut b[0], backing.w, backing.h, &backing.rgba),
                    ]
                });
                app.set_fetch_tex(
                    platform,
                    &stem,
                    FetchTex {
                        title,
                        bare,
                        done: finished,
                        under,
                        w: set.done.w,
                        h: set.done.h,
                    },
                );
            }
        }
    }
}

#[derive(Default)]
struct UpdateText {
    shown: Option<(String, String)>,
    title: Option<TexId>,
    detail: Option<TexId>,
    scene: bool,
    versions: Option<(String, String)>,
    chips: [Option<TexId>; 4],
}

#[derive(Default)]
struct AboutFace {
    tex: Option<TexId>,
    battery: Option<u8>,
}

#[derive(Default)]
struct Clocks {
    faces: Vec<[[Option<TexId>; 2]; 2]>,
    shelf: Option<TexId>,
    picked: Option<slot_ui::ClockPicker>,
    shown: String,
    battery: String,
    battery_tex: Option<TexId>,
    platform: String,
    platform_tex: Option<TexId>,
    title: String,
    title_tex: Option<TexId>,
    ink: [u8; 3],
}

struct ShelfLook {
    slot: [[TexId; 2]; 2],
    ds_slot: Option<[[TexId; 2]; 2]>,
    plugged: [TexId; 2],
    bolt: [TexId; 2],
    port: [Option<TexId>; 2],
    legend: [Vec<(TexId, u32)>; 2],
    applied: Option<(bool, bool, bool)>,
}

#[derive(Default)]
struct Switcher {
    open: bool,
    titled: Option<String>,
    shot: Option<TexId>,
    loader: Option<ShotLoader>,
    opened: u64,
    window: Vec<std::path::PathBuf>,
    thumbs: std::collections::HashMap<std::path::PathBuf, TexId>,
    spare: Vec<TexId>,
    full_wanted: Option<std::path::PathBuf>,
    full_pending: Option<std::path::PathBuf>,
    full_asked: Option<std::time::Instant>,
    first_window: Option<std::time::Instant>,
    counted: Option<String>,
    count: Option<TexId>,
    prompted: Option<(String, &'static str)>,
    prompt_title: Option<TexId>,
    prompt_detail: Option<TexId>,
}

fn save_headers_when_changed(root: std::path::PathBuf) {
    let _ = std::thread::Builder::new()
        .name("slot-headers".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(10));
            if slot_store::header_index::dirty() {
                if let Err(e) = slot_store::header_index::save(&root) {
                    eprintln!("slot: header index: {e}");
                }
            }
        });
}

impl Frontend {
    pub fn boot(platform: Box<dyn Platform>) -> Self {
        Self::boot_with(platform, crate::audio::opened_sink())
    }

    pub fn boot_with(platform: Box<dyn Platform>, sink: Box<dyn crate::audio::AudioSink>) -> Self {
        let now = Instant::now();
        let cart_faces = CartFaces::spawn(platform.root().to_path_buf());
        save_headers_when_changed(platform.root().to_path_buf());
        let lid_root = platform.root().to_path_buf();
        let mut session = Session::boot_with(platform.root().to_path_buf(), sink);
        session
            .app_mut()
            .set_update_jobs(crate::update::update_jobs(platform.root().to_path_buf()));
        session
            .app_mut()
            .set_art_jobs(crate::art::jobs::art_jobs(platform.root().to_path_buf()));
        #[cfg(feature = "romm")]
        session
            .app_mut()
            .set_romm_jobs(crate::romm::romm_jobs(platform.root().to_path_buf(), crate::romm::http_api));
        session.app_mut().set_wifi(wifi_worker(platform.root()));
        #[cfg(feature = "device")]
        session
            .app_mut()
            .set_wifi_probe(std::path::PathBuf::from("/sys/class/net/wlan0"));
        session.app_mut().connect_wifi();
        session.app_mut().require_fetch_faces();
        if let (Some(target), Some(version)) = (
            crate::baseos::var("BASEOS_TARGET"),
            crate::baseos::var("BASEOS_VERSION"),
        ) {
            crate::update::clean_baseos(platform.root(), &target, &version);
        }
        crate::boot_time::mark("session");
        let stale_check =
            Self::check_labels(platform.root(), session.app().carts().cloned().collect());
        session
            .app_mut()
            .set_power(Power::new(platform, DOZE_TIMEOUT));
        Frontend {
            session,
            start: now,
            last: now,
            draws: Vec::new(),
            ds: Default::default(),
            polaroid_texes: Vec::new(),
            title_tex: None,
            faces: FaceBuilder::spawn(lid_root),
            cart_faces,
            rebuild: None,
            stale_check,
            ui_faces: None,
            rebuild_faces: RebuildFaces::default(),
            link_art: LinkArtBuilder::spawn(),
            link_art_done: false,
            core_asked: None,
            core_tex: CoreTextures::default(),
            core_built: None,
            undo_tex: None,
            switcher: Switcher::default(),
            guide_page: GuidePage::default(),
            clocks: Clocks::default(),
            box_text: BoxTextSync::default(),
            box_art: BoxArtSync::default(),
            about: AboutFace::default(),
            update_text: UpdateText::default(),
            fetch_sync: FetchSync::new(),
            studio_sync: StudioSync::new(),
            print_tex: None,
            gift_boxes: Default::default(),
            print_stage: None,
            print_stage_asked: false,
            quick_clock: QuickClock::default(),
            quick_network: QuickClock::default(),
            quick_romm: Default::default(),
            quick_kid_shelf: Default::default(),
            networks: Default::default(),
            quick_palette: QuickPalette::default(),
            wallpaper: WallpaperSync::default(),
            picking_faces: PickingSync::default(),
            game_bezel: BezelSync::default(),
            overlay: OverlaySync::default(),
            look: None,
            grounds: None,
            link_shot: None,
            link_shot_size: (0, 0),
            link_shot_shown: false,
        }
    }

    fn upload_paper(compositor: &mut Compositor) {
        let mut dec = png::Decoder::new(std::io::Cursor::new(PAPER));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let Ok(mut reader) = dec.read_info() else {
            return;
        };
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let Ok(info) = reader.next_frame(&mut buf) else {
            return;
        };
        if info.color_type != png::ColorType::Grayscale || info.width != info.height {
            return;
        }
        compositor.set_paper(info.width, &buf[..info.buffer_size()]);
    }

    pub fn upload_faces(&mut self, compositor: &mut Compositor) {
        self.upload_boot_faces(compositor);
        if let Some(faces) = self.ui_faces.take().and_then(|rx| rx.recv().ok()) {
            self.place_ui_faces(compositor, faces);
        }
    }

    pub fn upload_boot_faces(&mut self, compositor: &mut Compositor) {
        Self::upload_paper(compositor);
        crate::boot_time::mark("paper");
        if self.rebuild.is_none() {
            self.cart_faces.sync(self.session.app_mut(), compositor);
        }
        crate::boot_time::mark("cart faces");
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("slot-ui-faces".into())
            .spawn(move || {
                let _ = tx.send(UiFaces::build());
            });
        match spawned {
            Ok(_) => self.ui_faces = Some(rx),
            Err(_) => self.place_ui_faces(compositor, UiFaces::build()),
        }
        let icons = Icon::ALL
            .iter()
            .map(|i| {
                let f = icon_face(*i, HUD_ICON_PX, HUD_INK);
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        self.session.app_mut().set_icon_faces(icons);
        let alert = icon_face(Icon::Alert, ALERT_PX, ALERT_INK);
        let alert = compositor.create_texture(alert.w, alert.h, &alert.rgba);
        self.session.app_mut().set_alert_face(alert);
        let legend = legend_faces(compositor, &LEGEND);
        self.session.app_mut().set_legend_faces(legend);
        let shot_hints = slot_ui::SHOT_HINTS
            .iter()
            .map(|&(key, label)| {
                let f = match key == slot_ui::PAN_KEY {
                    true => slot_ui::pad_hint_face(label),
                    false => hint_face(key, label),
                };
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        self.session.app_mut().set_shot_hint_faces(shot_hints);
        let guide_hints = slot_ui::GUIDE_HINTS
            .iter()
            .chain([&slot_ui::GUIDE_HELP_HINT])
            .map(|&(key, label)| {
                let f = match key == slot_ui::PAN_KEY {
                    true => slot_ui::pad_hint_face(label),
                    false => hint_face(key, label),
                };
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        self.session.app_mut().set_guide_hint_faces(guide_hints);
        let help = slot_ui::guide_help_face();
        let help = (
            compositor.create_texture(help.w, help.h, &help.rgba),
            help.w,
            help.h,
        );
        let title = title_face(slot_ui::GUIDE_HELP_TITLE);
        let title = compositor.create_texture(title.w, title.h, &title.rgba);
        self.session.app_mut().set_guide_help_face(help, title);
        let tick = slot_ui::tick_cap_face();
        let tick = (
            compositor.create_texture(tick.w, tick.h, &tick.rgba),
            tick.w,
            tick.h,
        );
        self.session.app_mut().set_shot_tick_face(tick);
        let clock_legends = [false, true].map(|light| {
            slot_ui::clock_legend_faces(footer_ink(light))
                .map(|f| (compositor.create_texture(f.w, f.h, &f.rgba), f.w))
        });
        self.session.app_mut().set_clock_legend_faces(clock_legends);
        let title = menu_face("Set the clock");
        let tex = compositor.create_texture(title.w, title.h, &title.rgba);
        self.session
            .app_mut()
            .set_clock_title_face((tex, title.w, title.h));
        let picking = [false, true].map(|light| {
            slot_ui::picking_legend_faces(footer_ink(light))
                .map(|f| (compositor.create_texture(f.w, f.h, &f.rgba), f.w))
        });
        self.session.app_mut().set_picking_faces(picking);
        let heart = slot_ui::heart_face(slot_ui::HEART_SCALE, slot_ui::HEART_INK);
        let id = compositor.create_texture_nearest(heart.w, heart.h, &heart.rgba);
        self.session.app_mut().set_heart_face(id, heart.w, heart.h);
        let shadow = cart_shadow();
        let id = compositor.create_texture(shadow.w, shadow.h, &shadow.rgba);
        self.session.app_mut().set_cart_shadow(id);
        let notched = gb_cart_shadow(GbShell::Notched);
        let notched = compositor.create_texture(notched.w, notched.h, &notched.rgba);
        let rounded = gb_cart_shadow(GbShell::Rounded);
        let rounded = compositor.create_texture(rounded.w, rounded.h, &rounded.rgba);
        self.session.app_mut().set_gb_cart_shadows(notched, rounded);
        let board = slot_ui::cart_backing(None);
        let board = compositor.create_texture(board.w, board.h, &board.rgba);
        self.session.app_mut().set_cart_backing(board);
        let notched = slot_ui::cart_backing(Some(GbShell::Notched));
        let notched = compositor.create_texture(notched.w, notched.h, &notched.rgba);
        let rounded = slot_ui::cart_backing(Some(GbShell::Rounded));
        let rounded = compositor.create_texture(rounded.w, rounded.h, &rounded.rgba);
        self.session
            .app_mut()
            .set_gb_cart_backings(notched, rounded);
        let slot = {
            let mut upload =
                |bytes: &[u8]| compositor.create_texture(slot_ui::SLOT_W, slot_ui::SLOT_H, bytes);
            [
                [upload(slot_back()), upload(slot_front())],
                [upload(slot_back_light()), upload(slot_front_light())],
            ]
        };
        #[cfg(feature = "nds")]
        let ds_slot = {
            let mut upload =
                |bytes: &[u8]| compositor.create_texture(slot_ui::SLOT_W, slot_ui::SLOT_H, bytes);
            Some([
                [upload(slot_ui::ds_slot_back()), upload(slot_ui::ds_slot_front())],
                [upload(slot_ui::ds_slot_back_light()), upload(slot_ui::ds_slot_front_light())],
            ])
        };
        #[cfg(not(feature = "nds"))]
        let ds_slot = None;
        let plugged = [
            compositor.create_texture(slot_ui::SLOT_W, slot_ui::SLOT_H, slot_ui::slot_plugged()),
            compositor.create_texture(
                slot_ui::SLOT_W,
                slot_ui::SLOT_H,
                slot_ui::slot_plugged_light(),
            ),
        ];
        let dark_bolt = icon_face(Icon::Charging, BOLT_PX, footer_ink(false));
        let light_bolt = icon_face(Icon::Charging, BOLT_PX, footer_ink(true));
        let bolt = [
            compositor.create_texture(dark_bolt.w, dark_bolt.h, &dark_bolt.rgba),
            compositor.create_texture(light_bolt.w, light_bolt.h, &light_bolt.rgba),
        ];
        let wifi = [false, true].map(|light| {
            [0, 1, 2, 3].map(|bars| {
                let f = slot_ui::wifi_face(bars, footer_ink(light));
                (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
            })
        });
        self.session.app_mut().set_wifi_faces(wifi);
        let kid = [false, true].map(|light| {
            let f = icon_face(Icon::Kid, slot_ui::KID_ICON_PX, footer_ink(light));
            (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
        });
        self.session.app_mut().set_kid_icon_faces(kid);
        self.look = Some(ShelfLook {
            slot,
            ds_slot,
            plugged,
            bolt,
            port: [None, None],
            legend: [Vec::new(), Vec::new()],
            applied: None,
        });
        self.apply_look();
        crate::boot_time::mark("shelf faces");
        self.upload_ground(compositor);
    }

    fn upload_ground(&mut self, compositor: &mut Compositor) {
        let Some(rgba) = slot_ui::default_background() else {
            return;
        };
        let ground = compositor.create_texture(OUT_W, OUT_H, rgba);
        self.session.app_mut().set_ground(ground);
        let blur = slot_ui::blur_background().map(|rgba| {
            let id = compositor.create_texture(OUT_W, OUT_H, rgba);
            self.session.app_mut().set_blur(id);
            id
        });
        self.grounds = Some((ground, blur, (OUT_W, OUT_H)));
    }

    fn sync_grounds(&mut self, compositor: &mut Compositor) {
        let Some((ground, blur, size)) = self.grounds else {
            return;
        };
        let want = self.session.app().wallpaper_size();
        if want == size || (size == (OUT_W, OUT_H) && slot_ui::reference_ground_covers(want)) {
            return;
        }
        if let Some(rgba) = slot_ui::default_background_at(want.0, want.1) {
            compositor.update_texture(ground, want.0, want.1, &rgba);
        }
        if let (Some(id), Some(rgba)) = (blur, slot_ui::blur_background_at(want.0, want.1)) {
            compositor.update_texture(id, want.0, want.1, &rgba);
        }
        self.session.app_mut().set_ground_width(want.0);
        self.grounds = Some((ground, blur, want));
    }

    fn apply_look(&mut self) {
        let light = self.session.app().light();
        let ready = self.look.as_ref().map(|look| {
            let i = usize::from(light);
            let legend_ready = !look.legend[i].is_empty();
            let port_ready = look.port[i].is_some();
            (
                look.applied == Some((light, legend_ready, port_ready)),
                look.slot[i],
                look.plugged[i],
                look.bolt[i],
                look.bolt[0],
                look.port[i],
                look.legend[i].clone(),
                legend_ready,
                port_ready,
            )
        });
        let Some((done, slot, plugged, bolt, hud_bolt, port, legend, legend_ready, port_ready)) =
            ready
        else {
            return;
        };
        if done {
            return;
        }
        self.session.app_mut().set_slot_faces(slot[0], slot[1]);
        if let Some(ds) = self.look.as_ref().and_then(|look| look.ds_slot) {
            let ds = ds[usize::from(light)];
            self.session.app_mut().set_ds_slot_faces(ds[0], ds[1]);
        }
        self.session.app_mut().set_plugged_face(plugged);
        if let Some(tex) = self.look.as_ref().map(|look| look.plugged[1]) {
            self.session.app_mut().set_printer_face(tex);
        }
        self.session.app_mut().set_bolt_face(bolt);
        self.session.app_mut().set_hud_bolt_face(hud_bolt);
        if let Some(tex) = port {
            self.session.app_mut().set_link_port(tex);
        }
        if legend_ready {
            self.session.app_mut().set_link_legend_faces(legend);
        }
        if let Some(look) = &mut self.look {
            look.applied = Some((light, legend_ready, port_ready));
        }
    }

    fn step_print_stage(&mut self, compositor: &mut Compositor) {
        let app = self.session.app();
        if !self.print_stage_asked && (matches!(app.phase(), Phase::Shelf) || app.server_open()) {
            self.print_stage_asked = true;
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("slot-print-stage".into())
                .spawn(move || {
                    let _ = tx.send(crate::print_scene::stage_art());
                });
            if let Err(e) = spawned {
                eprintln!("slot: print stage: worker thread failed to start: {e}");
            }
            self.print_stage = Some(rx);
        }
        let Some(rx) = &self.print_stage else {
            return;
        };
        match rx.try_recv() {
            Ok(art) => {
                self.print_stage = None;
                self.print_tex = art.and_then(|art| {
                    crate::print_scene::PrintTextures::upload(compositor, &art)
                        .inspect_err(|e| eprintln!("slot: server: {e}"))
                        .ok()
                });
                if self.print_tex.is_none() {
                    self.session.app_mut().server_art_failed();
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.print_stage = None,
        }
    }

    fn poll_ui_faces(&mut self, compositor: &mut Compositor) {
        let Some(rx) = &self.ui_faces else {
            return;
        };
        match rx.try_recv() {
            Ok(faces) => {
                self.ui_faces = None;
                self.place_ui_faces(compositor, faces);
                crate::boot_time::mark("ui faces");
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.ui_faces = None,
        }
    }

    fn place_ui_faces(&mut self, compositor: &mut Compositor, faces: UiFaces) {
        let mut cart = |f: CartFace| compositor.create_texture(f.w, f.h, &f.rgba);
        let link_badges = faces.link_badges.into_iter().map(&mut cart).collect();
        let toasts = faces.toasts.into_iter().map(&mut cart).collect();
        let sockets = faces.sockets.into_iter().map(&mut cart).collect();
        let chips = faces.chips.into_iter().map(&mut cart).collect();
        let blank = cart(faces.blank_chip);
        let gb_sockets = faces.gb_sockets.into_iter().map(&mut cart).collect();
        let gb_chips = faces.gb_chips.into_iter().map(&mut cart).collect();
        let gb_blank = cart(faces.gb_blank_chip);
        let shadow = cart(faces.chip_shadow);
        let mut up = |f: UndoFace| (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h);
        let lines = faces.shutdown.into_iter().map(&mut up).collect();
        let repair = faces.repair.into_iter().map(&mut up).collect();
        let reset = faces.reset.into_iter().map(&mut up).collect();
        let delete = faces.delete.into_iter().map(&mut up).collect();
        let kid = faces.kid.into_iter().map(&mut up).collect();
        let delete_shot = faces.delete_shot.into_iter().map(&mut up).collect();
        let labels = faces
            .quick_labels
            .into_iter()
            .map(|l| l.map(&mut up))
            .collect();
        let pointer = up(faces.quick_pointer);
        let values = faces
            .quick_values
            .into_iter()
            .map(|v| v.map(&mut up))
            .collect();
        let descriptions = faces.descriptions.map(&mut up);
        let mut pack_legend = |faces: [UndoFace; 8]| {
            faces.map(|f| {
                let (tex, w, _) = up(f);
                (tex, w)
            })
        };
        let legend = pack_legend(faces.quick_legend);
        let legend_light = pack_legend(faces.quick_legend_light);
        let roles = faces.link_roles.into_iter().map(&mut up).collect();
        let linked = up(faces.linked);
        let mut pack = |faces: Vec<UndoFace>| -> Vec<(TexId, u32)> {
            faces
                .into_iter()
                .map(|f| {
                    let (tex, w, _) = up(f);
                    (tex, w)
                })
                .collect()
        };
        let update = pack(faces.update_legends.into());
        let link_legend = pack(faces.link_legend);
        let link_legend_light = pack(faces.link_legend_light);
        let core_legend = pack(faces.core_legend);
        let core_legend_light = pack(faces.core_legend_light);
        let steps = faces.link_steps.into_iter().map(&mut up).collect();
        let fails = faces.link_fails.into_iter().map(&mut up).collect();
        if let Some(look) = &mut self.look {
            look.legend = [link_legend.clone(), link_legend_light];
            look.applied = None;
        }
        let app = self.session.app_mut();
        app.set_link_badge_faces(link_badges);
        app.set_shutdown_faces(lines);
        app.set_repair_faces(repair);
        if let Ok(update) = <[(TexId, u32); 3]>::try_from(update) {
            app.set_update_legends(update);
        }
        app.set_reset_faces(reset);
        app.set_kid_faces(kid);
        app.set_delete_faces(delete);
        app.set_delete_shot_faces(delete_shot);
        app.set_quick_menu_faces(QuickMenuFaces {
            labels,
            pointer,
            values,
            legend,
            legend_light,
            descriptions,
        });
        app.set_core_part_faces(sockets, chips, blank, shadow);
        app.set_core_gb_part_faces(gb_sockets, gb_chips, gb_blank);
        app.set_core_legend_faces(core_legend, core_legend_light);
        app.set_link_menu_faces(roles);
        app.set_link_linked_face(linked);
        app.set_link_legend_faces(link_legend);
        app.set_link_step_faces(steps);
        app.set_link_fail_faces(fails);
        app.set_toast_faces(toasts);
    }

    pub fn upload_bezel(&mut self, compositor: &mut Compositor, panel: (u32, u32)) {
        self.session.app_mut().set_panel(panel);
        if !slot_gfx::framed(panel) {
            return;
        }
        let Some(path) = self
            .session
            .app()
            .root()
            .and_then(|root| crate::bezel::pick(root, panel))
        else {
            return;
        };
        let (w, h) = self.session.app().game_bezel_size();
        if let Some(rgba) = bezel_face(&path, w, h) {
            let tex = compositor.create_texture(w, h, &rgba);
            self.session.app_mut().set_sp_strip(Some(tex));
            eprintln!("slot: bezel {}", path.display());
        }
    }

    pub fn render(&mut self, compositor: &mut Compositor, window: (u32, u32)) {
        self.session.app_mut().set_panel(window);
        compositor.fit(window);
        self.compose(compositor);
        compositor.end_frame(window);
    }

    fn grab_link_shot(&mut self, compositor: &mut Compositor) {
        let open = self.session.app().game_menu_open();
        if !open {
            if self.link_shot_shown {
                self.link_shot_shown = false;
                self.session.app_mut().set_link_shot(None);
            }
            return;
        }
        if self.link_shot_shown {
            return;
        }
        let (w, h) = compositor.frame_size();
        if w == 0 || h == 0 {
            return;
        }
        let rgba = compositor.read_frame();
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        let tex = match self.link_shot {
            Some(id) if self.link_shot_size == (w, h) => {
                compositor.update_texture(id, w, h, &rgba);
                id
            }
            _ => {
                let id = compositor.create_texture_nearest(w, h, &rgba);
                self.link_shot = Some(id);
                self.link_shot_size = (w, h);
                id
            }
        };
        self.session.app_mut().set_link_shot(Some(tex));
        self.link_shot_shown = true;
    }

    pub fn bottom_source(&self) -> crate::app::BottomSource {
        self.session.app().bottom_source()
    }

    pub fn set_two_panels(&mut self, on: bool) {
        self.session.app_mut().set_two_panels(on);
    }

    pub fn render_screen(&mut self, compositor: &mut Compositor, window: (u32, u32), screen: Screen) {
        if screen == Screen::Top {
            self.render(compositor, window);
            return;
        }
        self.session.app_mut().set_panel(window);
        compositor.fit(window);
        compositor.begin_frame();
        self.draw_screen_list(compositor, screen);
        compositor.end_frame(window);
    }

    pub fn compose(&mut self, compositor: &mut Compositor) {
        if self.prepare(compositor) {
            self.draw_screen_list(compositor, Screen::Top);
        }
    }

    fn draw_screen_list(&mut self, compositor: &mut Compositor, screen: Screen) {
        self.draws.clear();
        self.session.app().draw_screen(screen, &mut self.draws);
        if screen == Screen::Top || self.draws.iter().any(|d| matches!(d, Draw::Game)) {
            self.ds.swap_game(&mut self.draws, &self.session, compositor, screen);
        }
        compositor.draw_list(&self.draws);
        if screen == Screen::Top {
            if let Some((w, h, rgba)) = compositor.take_capture() {
                self.session.app_mut().shot_taken(w, h, rgba);
            }
        }
    }

    fn prepare(&mut self, compositor: &mut Compositor) -> bool {
        compositor.set_blue_light(self.session.app().blue_light());
        compositor.set_dim(self.session.app().dim());
        compositor.set_saturation(self.session.app().saturation());
        compositor.set_contrast(self.session.app().contrast());
        compositor.set_picture(self.session.app().picture_rect());
        compositor.set_screen_effect(self.session.app().screen_effect());
        compositor.set_shake(self.session.app().screen_shake());
        compositor.set_screen_power(self.session.app().screen_power());
        compositor.set_screen_footer(self.session.app().screen_footer());
        compositor.set_game_source_rect(self.session.app().source_rect());
        compositor.set_game_layout(self.session.app().game_layout());
        compositor.set_placeholder_bezel(self.session.app().placeholder_bezel());
        self.grab_link_shot(compositor);
        compositor.begin_frame();
        self.sync_grounds(compositor);
        if self.draw_rebuild(compositor) {
            return false;
        }
        if let Some([r, g, b]) = self.session.app().blank_panel() {
            let px = (slot_gfx::SRC_W * slot_gfx::SRC_H) as usize;
            compositor.upload_game(&[b, g, r, 0xff].repeat(px));
        } else if let Some(frame) = (!crate::ds_view::seated(&self.session))
            .then(|| self.session.frame())
            .flatten()
        {
            compositor.upload_game(&frame);
            crate::latency::taken();
        }
        self.cart_faces.sync(self.session.app_mut(), compositor);
        sync_clock(self.session.app_mut(), compositor, &mut self.clocks);
        sync_box_text(self.session.app_mut(), compositor, &mut self.box_text);
        sync_box_art(self.session.app_mut(), compositor, &mut self.box_art);
        sync_binding_faces(self.session.app_mut(), compositor);
        sync_about(self.session.app_mut(), compositor, &mut self.about);
        sync_update(self.session.app_mut(), compositor, &mut self.update_text);
        sync_fetch_art(self.session.app_mut(), compositor, &mut self.fetch_sync);
        sync_studio(self.session.app_mut(), compositor, &mut self.studio_sync);
        self.step_print_stage(compositor);
        sync_server(self.session.app_mut(), compositor, self.print_tex);
        self.gift_boxes.sync(self.session.app_mut(), compositor);
        let stale = self.session.app_mut().take_stale_labels();
        if !stale.is_empty() {
            if let Some(root) = self.session.app().root().map(|r| r.to_path_buf()) {
                self.rebuild = Some(Rebuild::start(&root, stale));
            }
        }
        if self.session.app_mut().take_rescanned() {
            if let Some(root) = self.session.app().root() {
                label_cache::relearn_stamps(root);
            }
            self.wallpaper.image = None;
            self.wallpaper.label.clear();
            self.wallpaper.shelf_asked.clear();
            self.game_bezel.image = None;
            self.game_bezel.labels = Default::default();
            self.overlay.want = None;
            self.overlay.labels = Default::default();
        }
        sync_quick_clock(self.session.app_mut(), compositor, &mut self.quick_clock);
        sync_quick_network(self.session.app_mut(), compositor, &mut self.quick_network);
        sync_quick_romm(self.session.app_mut(), compositor, &mut self.quick_romm);
        sync_quick_kid_shelf(self.session.app_mut(), compositor, &mut self.quick_kid_shelf);
        self.networks.sync(self.session.app_mut(), compositor);
        sync_quick_palette(self.session.app_mut(), compositor, &mut self.quick_palette);
        sync_wallpaper(self.session.app_mut(), compositor, &mut self.wallpaper);
        sync_picking(self.session.app_mut(), compositor, &mut self.picking_faces);
        sync_game_bezel(self.session.app_mut(), compositor, &mut self.game_bezel);
        compositor.set_game_bezel(self.session.app().shown_game_bezel());
        sync_overlay(self.session.app_mut(), compositor, &mut self.overlay, self.ds.showing_top());
        compositor.set_sp_frame(self.session.app().picture_on_top());
        sync_core_picker(
            self.session.app_mut(),
            compositor,
            &self.faces,
            &mut self.core_asked,
            &mut self.core_tex,
            &mut self.core_built,
        );
        if !self.link_art_done {
            if let Some(art) = self.link_art.take() {
                let mut up = |f: &slot_ui::CartFace| Sprite {
                    tex: compositor.create_texture(f.w, f.h, &f.rgba),
                    w: f.w,
                    h: f.h,
                };
                let port_dark = up(&art.port);
                let port_light = up(&art.port_light);
                if let Some(look) = &mut self.look {
                    look.port = [Some(port_dark.tex), Some(port_light.tex)];
                    look.applied = None;
                }
                let sprites = LinkSprites {
                    port: port_dark,
                    plug_host: up(&art.plug_host),
                    plug_join: up(&art.plug_join),
                    adapter: up(&art.adapter),
                    arcs_right: [
                        up(&art.arcs_right[0]),
                        up(&art.arcs_right[1]),
                        up(&art.arcs_right[2]),
                    ],
                    arcs_left: [
                        up(&art.arcs_left[0]),
                        up(&art.arcs_left[1]),
                        up(&art.arcs_left[2]),
                    ],
                    clicks: up(&art.clicks),
                    arrow_left: up(&art.arrow_left),
                    arrow_right: up(&art.arrow_right),
                };
                self.session.app_mut().set_link_sprites(sprites);
                self.link_art_done = true;
            }
        }
        self.apply_look();
        sync_switcher(
            self.session.app_mut(),
            compositor,
            Faces {
                pool: &mut self.polaroid_texes,
                title: &mut self.title_tex,
                undo: &mut self.undo_tex,
            },
            &mut self.switcher,
        );
        sync_guide(self.session.app_mut(), compositor, &mut self.guide_page);
        true
    }

    pub fn drive_emulator(&mut self) {
        self.session.set_driven(true);
    }

    pub fn step_emulator(&self, present: Duration, timeout: Duration) -> bool {
        self.session.step_emulator(present, timeout)
    }

    pub fn rebuilding_labels(&self) -> bool {
        self.rebuild.is_some()
    }

    pub fn rebuild_drawn(&self) -> bool {
        self.rebuild_faces.shown.is_some()
    }

    fn check_labels(
        root: &std::path::Path,
        carts: Vec<slot_store::Cart>,
    ) -> Option<std::sync::mpsc::Receiver<Vec<slot_store::Cart>>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let root = root.to_path_buf();
        std::thread::Builder::new()
            .name("slot-label-check".into())
            .spawn(move || {
                let _ = tx.send(label_cache::stale(&root, carts.iter()));
                label_cache::learn_stamps(&root);
            })
            .ok()
            .map(|_| rx)
    }

    fn poll_label_check(&mut self) {
        let Some(rx) = &self.stale_check else {
            return;
        };
        let stale = match rx.try_recv() {
            Ok(stale) => stale,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Vec::new(),
        };
        self.stale_check = None;
        let Some(root) = self.session.app().root().map(|r| r.to_path_buf()) else {
            return;
        };
        if !stale.is_empty() {
            eprintln!("slot: label cache: {} new labels", stale.len());
            self.rebuild = Some(Rebuild::start(&root, stale));
        }
    }

    fn draw_rebuild(&mut self, compositor: &mut Compositor) -> bool {
        self.poll_label_check();
        self.poll_ui_faces(compositor);
        let Some(rebuild) = &mut self.rebuild else {
            return false;
        };
        if rebuild.stalled() && !rebuild.finished() {
            eprintln!(
                "slot: label cache: no label finished for a while, going on at {} / {}",
                rebuild.done(),
                rebuild.total()
            );
            self.rebuild = None;
            self.last = Instant::now();
            return false;
        }
        if rebuild.finished() {
            self.rebuild = None;
            self.last = Instant::now();
            return false;
        }
        if !rebuild.on_screen() {
            return false;
        }
        let faces = &mut self.rebuild_faces;
        let title = *faces.title.get_or_insert_with(|| {
            let f = rebuild_title_face("Caching New Labels");
            (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
        });
        let done = rebuild.done();
        if faces.shown != Some(done) {
            let f = rebuild_count_face(&format!("{done} / {}", rebuild.total()));
            faces.count_size = (f.w, f.h);
            upload(compositor, &mut faces.count, f);
            faces.shown = Some(done);
        }
        let screen = RebuildScreen {
            title: Some(title),
            count: faces
                .count
                .map(|tex| (tex, faces.count_size.0, faces.count_size.1)),
            fraction: done as f32 / rebuild.total().max(1) as f32,
        };
        self.draws.clear();
        screen.draw(self.session.app().canvas(), &mut self.draws);
        compositor.draw_list(&self.draws);
        true
    }

    pub fn advance(&mut self, input: &mut dyn InputSource) {
        let now = self.now();
        let events = input.poll(now);
        if self.rebuild.as_ref().is_some_and(Rebuild::on_screen) {
            return;
        }
        self.session.feed(events, now);
        let dt = self.last.elapsed().as_secs_f32();
        self.last = Instant::now();
        self.session.update(dt);
    }

    pub fn gallery_decodes(&self) -> usize {
        self.switcher.loader.as_ref().map_or(0, ShotLoader::decoded)
    }

    pub fn gallery_textures(&self) -> usize {
        self.switcher.thumbs.len() + self.switcher.spare.len()
    }

    pub fn app(&self) -> &crate::app::App {
        self.session.app()
    }

    pub fn ds_overlay(&self) -> Option<&crate::overlay::DecodedOverlay> {
        self.overlay.ds.as_ref()
    }

    pub fn ds_frame(&self) -> Option<&[u8]> {
        self.ds.last_frame()
    }

    pub fn ds_direct(&self) -> bool {
        crate::ds_view::direct(&self.session)
    }

    pub fn screen_power(&self) -> f32 {
        self.session.app().screen_power()
    }

    pub fn ds_menu_below(&self) -> bool {
        crate::ds_view::playing(&self.session) && !crate::ds_view::direct(&self.session)
    }

    pub fn frames_published(&self) -> u64 {
        self.session.frames_published()
    }

    pub fn set_ds_touch(&self, at: Option<(u8, u8)>) {
        crate::ds_view::set_touch(&self.session, at);
    }

    pub fn app_mut(&mut self) -> &mut crate::app::App {
        self.session.app_mut()
    }

    pub fn set_update_jobs(&mut self, jobs: Box<dyn crate::update::UpdateJobs>) {
        self.session.app_mut().set_update_jobs(jobs);
    }

    pub fn set_art_jobs(&mut self, jobs: Box<dyn crate::art::jobs::ArtJobs>) {
        self.session.app_mut().set_art_jobs(jobs);
    }

    pub fn set_romm_jobs(&mut self, jobs: Box<dyn crate::romm::RommJobs>) {
        self.session.app_mut().set_romm_jobs(jobs);
    }

    pub fn set_left_stick(&mut self, present: bool) {
        self.session.app_mut().set_left_stick(present);
    }

    pub fn core_settling(&self) -> bool {
        self.session.core_settling()
    }

    pub fn advance_at(&mut self, input: &mut dyn InputSource, now: Millis, dt: f32) {
        let events = input.poll(now);
        self.session.feed(events, now);
        self.session.update(dt);
    }

    fn now(&self) -> Millis {
        self.start.elapsed().as_millis() as Millis
    }

    pub fn powering_off(&self) -> bool {
        self.session.app().ready_to_power_off()
    }

    pub fn restarting(&self) -> bool {
        self.session.app().ready_to_restart()
    }

    pub fn restart(&mut self) {
        self.session.close_audio();
        self.session.app_mut().restart();
    }

    pub fn relaunching(&self) -> bool {
        self.session.app().relaunching()
    }

    pub fn relaunch(&mut self) {
        self.session.close_audio();
        self.stop();
    }

    pub fn repairing(&self) -> bool {
        self.session.app().ready_to_repair()
    }

    pub fn repair(&mut self) {
        self.session.app_mut().repair();
    }

    pub fn poweroff(&mut self) {
        self.session.close_audio();
        self.session.app_mut().poweroff();
    }

    pub fn stop(&mut self) {
        self.session.app_mut().end_server();
        self.session.app_mut().flush_resume();
        self.session.app_mut().settle_saves();
        self.session.app_mut().clear_loading_mark();
    }
}

#[cfg(feature = "device")]
fn wifi_worker(root: &std::path::Path) -> crate::wifi::Wifi {
    crate::wifi::sp::device_wifi(root)
}

#[cfg(not(feature = "device"))]
fn wifi_worker(_root: &std::path::Path) -> crate::wifi::Wifi {
    crate::wifi::Wifi::spawn(Box::new(crate::wifi::Fake::joined()))
}

fn link_legend_faces(ink: [u8; 3]) -> Vec<UndoFace> {
    link_button_legends(ink)
}

fn legend_faces(compositor: &mut Compositor, legend: &[(&str, &str)]) -> Vec<TexId> {
    legend
        .iter()
        .map(|(key, label)| {
            let f = hint_face(key, label);
            compositor.create_texture(f.w, f.h, &f.rgba)
        })
        .collect()
}

struct UiFaces {
    link_badges: Vec<CartFace>,
    shutdown: Vec<UndoFace>,
    quick_labels: Vec<[UndoFace; 2]>,
    quick_pointer: UndoFace,
    quick_values: Vec<[UndoFace; 2]>,
    quick_legend: [UndoFace; 8],
    quick_legend_light: [UndoFace; 8],
    descriptions: [UndoFace; 2],
    sockets: Vec<CartFace>,
    chips: Vec<CartFace>,
    blank_chip: CartFace,
    gb_sockets: Vec<CartFace>,
    gb_chips: Vec<CartFace>,
    gb_blank_chip: CartFace,
    chip_shadow: CartFace,
    core_legend: Vec<UndoFace>,
    core_legend_light: Vec<UndoFace>,
    link_roles: Vec<UndoFace>,
    linked: UndoFace,
    link_legend: Vec<UndoFace>,
    link_legend_light: Vec<UndoFace>,
    link_steps: Vec<UndoFace>,
    link_fails: Vec<UndoFace>,
    toasts: Vec<CartFace>,
    repair: Vec<UndoFace>,
    reset: Vec<UndoFace>,
    kid: Vec<UndoFace>,
    delete: Vec<UndoFace>,
    delete_shot: Vec<UndoFace>,
    update_legends: [UndoFace; 3],
}

impl UiFaces {
    fn build() -> Self {
        UiFaces {
            link_badges: LinkBadge::FACES
                .iter()
                .map(|b| {
                    let (badge, ink) = (
                        b.badge().expect("a face has a glyph"),
                        b.colour().expect("and a colour"),
                    );
                    badge_face(badge, HUD_ICON_PX, ink)
                })
                .collect(),
            shutdown: PowerChoice::ALL
                .iter()
                .map(|c| {
                    menu_face(match c {
                        PowerChoice::Restart => "Restarting",
                        PowerChoice::PowerOff => "Powering Down",
                        PowerChoice::Repair => "Repairing SD Card",
                    })
                })
                .collect(),
            quick_labels: QuickRow::ALL
                .iter()
                .map(|r| [false, true].map(|lit| quick_label_face(*r, lit)))
                .collect(),
            quick_pointer: quick_pointer_face(),
            quick_values: QuickValue::ALL
                .iter()
                .map(|v| [false, true].map(|lit| quick_value_face(v.text(), lit)))
                .collect(),
            quick_legend: quick_legend_faces(footer_ink(false)),
            quick_legend_light: quick_legend_faces(footer_ink(true)),
            descriptions: [
                quick_description_face(&["Supports single button and MENU+button."]),
                quick_description_face(&["Supports single button and two button chords."]),
            ],
            sockets: slot_store::Core::ALL
                .iter()
                .map(|c| socket_face(*c))
                .collect(),
            chips: slot_store::Core::ALL
                .iter()
                .map(|c| chip_face(Some(*c)))
                .collect(),
            blank_chip: chip_face(None),
            gb_sockets: slot_store::Core::ALL
                .iter()
                .map(|c| slot_ui::gb_socket_face(*c))
                .collect(),
            gb_chips: slot_store::Core::ALL
                .iter()
                .map(|c| slot_ui::gb_chip_face(Some(*c)))
                .collect(),
            gb_blank_chip: slot_ui::gb_chip_face(None),
            chip_shadow: chip_shadow_face(),
            core_legend: core_button_legends(footer_ink(false)),
            core_legend_light: core_button_legends(footer_ink(true)),
            link_roles: LinkRow::ALL.iter().map(|r| menu_face(r.text())).collect(),
            linked: menu_face("Linked"),
            link_legend: link_legend_faces(footer_ink(false)),
            link_legend_light: link_legend_faces(footer_ink(true)),
            link_steps: LinkStep::ALL.iter().map(|s| menu_face(s.line())).collect(),
            link_fails: LinkFail::SHOWN
                .iter()
                .map(|f| menu_face(f.line()))
                .collect(),
            toasts: Toast::all().into_iter().map(toast_face).collect(),
            update_legends: update_legends(footer_ink(false)),
            repair: {
                let [not_now, repair] = repair_legends(footer_ink(false));
                vec![
                    menu_face("Repair the SD card?"),
                    menu_detail_face("slot restarts when it is done."),
                    not_now,
                    repair,
                ]
            },
            delete: {
                let [keep, delete] = delete_legends(footer_ink(false));
                vec![
                    menu_face("Delete this save state?"),
                    menu_detail_face("It cannot be brought back."),
                    keep,
                    delete,
                ]
            },
            kid: {
                let [cancel, on] = prompt_legends(footer_ink(false), "Cancel", "Turn On");
                vec![
                    menu_face("Turn on Kid Mode?"),
                    menu_detail_face("Settings and editing lock."),
                    menu_detail_face("To unlock, hold START, SELECT, L2 and R2"),
                    menu_detail_face("while turning on."),
                    cancel,
                    on,
                ]
            },
            delete_shot: {
                let [keep, delete] = delete_legends(footer_ink(false));
                vec![
                    menu_face("Delete this screenshot?"),
                    menu_detail_face("It cannot be brought back."),
                    keep,
                    delete,
                ]
            },
            reset: {
                let [cancel, reset] = prompt_legends(footer_ink(false), "Cancel", "Reset");
                vec![
                    menu_face("Reset all controls?"),
                    menu_detail_face("Every button goes back to its default."),
                    menu_face("Reset all shortcuts?"),
                    menu_detail_face("Every shortcut goes back to its default."),
                    menu_face("Reset all shelf keys?"),
                    menu_detail_face("Every shelf key goes back to its default."),
                    cancel,
                    reset,
                ]
            },
        }
    }
}

struct Faces<'a> {
    pool: &'a mut Vec<TexId>,
    title: &'a mut Option<TexId>,
    undo: &'a mut Option<TexId>,
}

fn sync_switcher(app: &mut App, compositor: &mut Compositor, texes: Faces, state: &mut Switcher) {
    if !matches!(app.phase(), Phase::Polaroids { .. }) {
        if state.open {
            close_gallery(app, state);
        }
        state.open = false;
        return;
    }
    if !state.open {
        state.open = true;
        state.titled = None;
        let faces: Vec<_> = app.polaroid_entries().iter().map(photo_face).collect();
        let ids = faces
            .iter()
            .enumerate()
            .map(|(i, f)| match texes.pool.get(i) {
                Some(id) => {
                    compositor.update_texture(*id, f.w, f.h, &f.rgba);
                    *id
                }
                None => {
                    let id = compositor.create_texture_nearest(f.w, f.h, &f.rgba);
                    texes.pool.push(id);
                    id
                }
            })
            .collect();
        app.set_polaroid_faces(ids);

        let label = app
            .undo_label()
            .map(|l| upload(compositor, texes.undo, hint_face("X", l)));
        app.set_undo_face(label);
    }
    if state.titled.as_deref() != app.polaroid_stamp() {
        state.titled = app.polaroid_stamp().map(str::to_string);
        let face = title_face(&app.polaroid_title(&format_stamp(app.wall_secs())));
        let id = upload(compositor, texes.title, face);
        app.set_polaroid_title_face(id);
    }
    sync_gallery(app, compositor, state);
}

#[derive(Default)]
struct GuidePage {
    tex: Option<TexId>,
    shown: Option<slot_ui::GuideView>,
    glyphs: slot_ui::GuideGlyphs,
}

fn sync_guide(app: &mut App, compositor: &mut Compositor, state: &mut GuidePage) {
    let canvas = app.canvas();
    let Some(guide) = app.guide() else {
        state.shown = None;
        return;
    };
    let view = guide.view(canvas);
    if state.shown != Some(view) {
        let face = guide.render(canvas, &mut state.glyphs);
        let tex = upload(compositor, &mut state.tex, face);
        state.shown = Some(view);
        app.set_guide_page_face(tex, view);
    }
}

fn close_gallery(app: &mut App, state: &mut Switcher) {
    if let Some(loader) = &mut state.loader {
        loader.cancel();
    }
    for (path, id) in state.thumbs.drain() {
        app.set_shot_thumb(&path, None);
        state.spare.push(id);
    }
    state.window.clear();
    state.full_wanted = None;
    state.full_pending = None;
    state.full_asked = None;
    state.first_window = None;
    state.counted = None;
    state.prompted = None;
}

fn sync_gallery(app: &mut App, compositor: &mut Compositor, state: &mut Switcher) {
    if app.shot_viewer().is_none() {
        if !state.window.is_empty() || !state.thumbs.is_empty() {
            close_gallery(app, state);
        }
        return;
    }
    if app.shot_opens() != state.opened {
        close_gallery(app, state);
        state.opened = app.shot_opens();
        state.loader.get_or_insert_with(ShotLoader::spawn).cancel();
        state.first_window = Some(std::time::Instant::now());
    }
    let Some(loader) = &mut state.loader else {
        return;
    };
    let canvas = app.canvas();
    let Some(v) = app.shot_viewer() else {
        return;
    };
    let count = v.count_text();
    let has_face = v.has_face();
    let window: Vec<std::path::PathBuf> = v
        .thumb_order(canvas)
        .into_iter()
        .map(|i| v.entries[i].path.clone())
        .collect();
    if window != state.window {
        let gone: Vec<_> = state
            .thumbs
            .keys()
            .filter(|p| !window.contains(p))
            .cloned()
            .collect();
        for path in gone {
            if let Some(id) = state.thumbs.remove(&path) {
                state.spare.push(id);
            }
            app.set_shot_thumb(&path, None);
        }
        let wanted = window
            .iter()
            .filter(|p| !state.thumbs.contains_key(*p))
            .cloned()
            .collect();
        let (cw, ch) = slot_ui::grid_cell(canvas);
        loader.queue_thumbs(wanted, cw.floor() as u32, ch.floor() as u32);
        state.window = window;
    }
    if state.counted.as_deref() != Some(count.as_str()) {
        let face = slot_ui::word_face(&count);
        let id = upload_rgba(compositor, &mut state.count, face.w, face.h, &face.rgba);
        app.set_shot_count_face(Some((id, face.w)));
        state.counted = Some(count);
    }
    let prompted = app.delete_shot_lines();
    if prompted != state.prompted {
        if let Some((title, detail)) = &prompted {
            let t = menu_face(title);
            let d = menu_detail_face(detail);
            let tid = upload_rgba(compositor, &mut state.prompt_title, t.w, t.h, &t.rgba);
            let did = upload_rgba(compositor, &mut state.prompt_detail, d.w, d.h, &d.rgba);
            app.set_delete_shot_text((tid, t.w, t.h), (did, d.w, d.h));
        }
        state.prompted = prompted;
    }
    let want = app.shot_wants_full().map(std::path::Path::to_path_buf);
    if want != state.full_wanted {
        state.full_wanted = want.clone();
        state.full_pending = None;
    }
    if let Some(path) = want.filter(|p| !has_face && state.full_pending.as_ref() != Some(p)) {
        loader.full(path.clone());
        state.full_pending = Some(path);
        state.full_asked = Some(std::time::Instant::now());
    }
    for l in loader.poll() {
        match l.want {
            Want::Thumb
                if state.window.contains(&l.path) && !state.thumbs.contains_key(&l.path) =>
            {
                let id = match state.spare.pop() {
                    Some(id) => {
                        compositor.update_texture(id, l.w, l.h, &l.rgba);
                        id
                    }
                    None => compositor.create_texture_nearest(l.w, l.h, &l.rgba),
                };
                state.thumbs.insert(l.path.clone(), id);
                app.set_shot_thumb(&l.path, Some((id, l.w, l.h)));
                if state.thumbs.len() == state.window.len() {
                    if let Some(at) = state.first_window.take() {
                        eprintln!(
                            "slot: gallery: first {} thumbs in {} ms",
                            state.thumbs.len(),
                            at.elapsed().as_millis()
                        );
                    }
                }
            }
            Want::Full if state.full_wanted.as_ref() == Some(&l.path) => {
                let id = match state.shot {
                    Some(id) => {
                        compositor.update_texture(id, l.w, l.h, &l.rgba);
                        id
                    }
                    None => {
                        let id = compositor.create_texture_nearest(l.w, l.h, &l.rgba);
                        state.shot = Some(id);
                        id
                    }
                };
                app.set_shot_face(Some((id, l.w, l.h)));
                state.full_pending = None;
                if let Some(at) = state.full_asked.take() {
                    eprintln!(
                        "slot: gallery: full shot in {} ms",
                        at.elapsed().as_millis()
                    );
                }
            }
            _ => {}
        }
    }
}

fn sync_binding_faces(app: &mut App, compositor: &mut Compositor) {
    if app.quick_menu().is_none() && app.in_game_menu().is_none() {
        return;
    }
    for text in app.binding_texts() {
        if app.has_binding_face(&text) {
            continue;
        }
        let faces = [false, true].map(|lit| {
            let f = match text.strip_prefix(crate::app::CLASH_MARK) {
                Some(bare) => quick_warning_value_face(bare, lit),
                None => quick_value_face(&text, lit),
            };
            (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
        });
        app.add_binding_face(text, faces);
    }
}

fn sync_clock(app: &mut App, compositor: &mut Compositor, clocks: &mut Clocks) {
    let ink = app.status_ink();
    if clocks.ink != ink {
        clocks.ink = ink;
        clocks.shown.clear();
        clocks.battery.clear();
    }
    let picked = app.picker().cloned();
    if picked != clocks.picked {
        if let Some(set) = picked.as_ref().map(|p| p.faces()) {
            let rows = set.rows.len();
            if clocks.faces.len() < rows {
                clocks.faces.resize(rows, [[None; 2]; 2]);
            }
            let rows = set
                .rows
                .into_iter()
                .zip(clocks.faces.iter_mut())
                .map(|([[ld, ll], [vd, vl]], [[sld, sll], [svd, svl]])| {
                    let mut put = |f: slot_ui::UndoFace, id: &mut Option<TexId>| {
                        let (w, h) = (f.w, f.h);
                        (upload(compositor, id, f), w, h)
                    };
                    [[put(ld, sld), put(ll, sll)], [put(vd, svd), put(vl, svl)]]
                })
                .collect();
            app.set_clock_faces(slot_ui::ClockFaces {
                rows,
                caret: set.caret,
            });
        }
        clocks.picked = picked;
    }
    let shown = time_text(app.wall_secs(), app.clock_24h());
    if shown != clocks.shown {
        let face = word_ink(&shown, clocks.ink);
        clocks.shown = shown;
        let w = face.w;
        let id = upload(compositor, &mut clocks.shelf, face);
        app.set_shelf_clock_face(id, w);
    }
    let platform_shown = app.slot_text().unwrap_or_default();
    let lost = !platform_shown.is_empty() && app.shelf_platform_missing();
    if platform_shown != clocks.platform || lost {
        clocks.platform = platform_shown.clone();
        if platform_shown.is_empty() {
            app.clear_shelf_platform();
        } else {
            let face = slot_ui::title_line_face(&platform_shown);
            let w = face.w;
            let id = upload(compositor, &mut clocks.platform_tex, face);
            app.set_shelf_platform_face(id, w);
        }
    }
    let title_shown = app.title_text().unwrap_or_default();
    if title_shown != clocks.title {
        clocks.title = title_shown.clone();
        if title_shown.is_empty() {
            app.clear_shelf_title();
        } else {
            let face = slot_ui::title_line_face(&title_shown);
            let w = face.w;
            let id = upload(compositor, &mut clocks.title_tex, face);
            app.set_shelf_title_face(id, w);
        }
    }
    let battery_shown = app
        .battery()
        .map(|b| format!("{}%", b.percent))
        .unwrap_or_default();
    if battery_shown != clocks.battery {
        clocks.battery = battery_shown.clone();
        if !battery_shown.is_empty() {
            let face = word_ink(&battery_shown, clocks.ink);
            let w = face.w;
            let id = upload(compositor, &mut clocks.battery_tex, face);
            app.set_battery_percent_face(id, w);
        }
    }
}

fn bezel_key(pick: &BezelPick, art: Option<BezelArt>, last: Option<BezelArt>) -> Option<BezelArt> {
    match pick {
        BezelPick::Default => art.or(last),
        _ => None,
    }
}

fn sync_game_bezel(app: &mut App, compositor: &mut Compositor, state: &mut BezelSync) {
    let platform = app.playing_platform();
    let pick = app.resolved_bezel(platform);
    let (w, h) = app.game_bezel_size();
    let last = state.image.as_ref().and_then(|image| image.2);
    let art = bezel_key(&pick, app.bezel_art(), last);
    let shown = (platform, pick, art, (w, h));
    if state.image.as_ref() != Some(&shown) {
        let rgba = match &shown.1 {
            BezelPick::None | BezelPick::Ags102 => None,
            BezelPick::Default => shown
                .2
                .and_then(|kind| default_game_bezel_face(platform == Console::Gbc, kind, w, h)),
            BezelPick::File(name) => app
                .root()
                .map(|root| {
                    root.join(crate::bezel::GAME_BEZELS)
                        .join(platform.dir_name())
                        .join(name)
                })
                .and_then(|path| game_bezel_face(&path, w, h)),
        };
        app.set_bezel_column(
            rgba.as_deref()
                .map(|rgba| slot_gfx::bezel_column(rgba, (w, h)))
                .unwrap_or_default(),
        );
        let tex = rgba.map(|rgba| match state.tex {
            Some(id) => {
                compositor.update_texture(id, w, h, &rgba);
                id
            }
            None => {
                let id = compositor.create_texture(w, h, &rgba);
                state.tex = Some(id);
                id
            }
        });
        app.set_game_bezel(tex);
        state.image = Some(shown);
    }
    if app.quick_menu().is_none() && app.in_game_menu().is_none() {
        return;
    }
    for (at, platform) in [Console::Gba, Console::Gb, Console::Gbc]
        .into_iter()
        .enumerate()
    {
        let label = app.resolved_bezel(platform).label();
        if state.labels[at] == label {
            continue;
        }
        let (dim, lit) = (
            quick_value_face(&label, false),
            quick_value_face(&label, true),
        );
        let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
        let dim = upload(compositor, &mut state.faces[at].0, dim);
        let lit = upload(compositor, &mut state.faces[at].1, lit);
        app.set_quick_bezel_faces(
            platform,
            (dim, dim_size.0, dim_size.1),
            (lit, lit_size.0, lit_size.1),
        );
        state.labels[at] = label;
    }
}

fn sync_overlay(app: &mut App, compositor: &mut Compositor, state: &mut OverlaySync, ds_top: bool) {
    let platform = app.playing_platform();
    let entry = app.resolved_overlay(platform);
    let want = Some((platform, entry.clone()));
    if state.want != want {
        state.want = want;
        state.serial += 1;
        let dir = app.root().map(|root| crate::overlay::folder(root, platform));
        match (entry, dir) {
            (Some(e), Some(dir)) => {
                let path = |name: &Option<String>| name.as_ref().map(|n| dir.join(n));
                state
                    .loader
                    .get_or_insert_with(crate::overlay::OverlayLoader::spawn)
                    .request(crate::overlay::OverlayRequest {
                        serial: state.serial,
                        tile: path(&e.tile),
                        both: path(&e.both),
                        top: path(&e.top),
                        bottom: path(&e.bottom),
                    });
            }
            _ => {
                state.shown = None;
                state.ds = Some(crate::overlay::DecodedOverlay {
                    serial: state.serial,
                    ..Default::default()
                });
            }
        }
    }
    if let Some(done) = state.loader.as_ref().and_then(|l| l.latest()) {
        if done.serial == state.serial {
            let tile = done
                .tile
                .as_ref()
                .map(|(w, h, rgba)| (upload_rgba(compositor, &mut state.tile, *w, *h, rgba), *w, *h));
            let two = crate::overlay::two_screens(platform);
            let (top, bottom) = match two {
                true => (done.top.as_ref().or(done.both.as_ref()), done.bottom.as_ref().or(done.both.as_ref())),
                false => (done.both.as_ref(), None),
            };
            let top = top.map(|(w, h, rgba)| (upload_rgba(compositor, &mut state.full, *w, *h, rgba), *w, *h));
            let bottom = bottom
                .map(|(w, h, rgba)| (upload_rgba(compositor, &mut state.full_bottom, *w, *h, rgba), *w, *h));
            state.shown = Some(ShownOverlay { tile, top, bottom });
            state.ds = Some(done);
        }
    }
    let ds_top = ds_top || !crate::overlay::two_screens(platform);
    compositor.set_overlay(state.shown.map(|s| slot_gfx::Overlay {
        full: if ds_top { s.top } else { s.bottom },
        tile: s.tile,
    }));
    if app.quick_menu().is_none() && app.in_game_menu().is_none() {
        return;
    }
    for platform in [
        Console::Gba,
        Console::Gb,
        Console::Gbc,
        #[cfg(feature = "nds")]
        Console::Nds,
    ] {
        let at = crate::app::overlay_slot(platform);
        let label = app.overlay_label(platform);
        if state.labels[at] == label {
            continue;
        }
        let (dim, lit) = (quick_value_face(&label, false), quick_value_face(&label, true));
        let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
        let dim = upload(compositor, &mut state.faces[at].0, dim);
        let lit = upload(compositor, &mut state.faces[at].1, lit);
        app.set_quick_overlay_faces(
            platform,
            (dim, dim_size.0, dim_size.1),
            (lit, lit_size.0, lit_size.1),
        );
        state.labels[at] = label;
    }
}

fn sync_shelf_wallpaper(app: &mut App, compositor: &mut Compositor, state: &mut WallpaperSync) {
    let size = app.wallpaper_size();
    let shown = app.shown_wallpaper();
    state.image = None;
    let Some(root) = app.root().map(|root| root.to_path_buf()) else {
        return;
    };
    let loader = state
        .loader
        .get_or_insert_with(crate::wallpaper::WallpaperLoader::spawn);
    let priming = state.shelf_primed != Some(size);
    if priming {
        state.shelf_primed = Some(size);
        if let Some(name) = &shown {
            if state.shelf_asked.insert((name.clone(), size)) {
                let path = root.join("Wallpapers").join(name);
                if let Some(rgba) = wallpaper_face(&path, size.0, size.1) {
                    let id = compositor.create_texture(size.0, size.1, &rgba);
                    state.shelf_faces.insert((name.clone(), size), id);
                }
            }
        }
    }
    let rest = if priming {
        app.shelf_wallpapers()
    } else {
        Vec::new()
    };
    for name in shown.iter().chain(&rest) {
        if state.shelf_asked.insert((name.clone(), size)) {
            loader.request(&root, name, size);
        }
    }
    if let Some(done) = loader.take() {
        let key = (done.name, done.size);
        match state.shelf_faces.get(&key) {
            Some(id) => compositor.update_texture(*id, key.1 .0, key.1 .1, &done.rgba),
            None => {
                let id = compositor.create_texture(key.1 .0, key.1 .1, &done.rgba);
                state.shelf_faces.insert(key, id);
            }
        }
    }
    if let Some(id) = shown.and_then(|name| state.shelf_faces.get(&(name, size))) {
        app.set_wallpaper(*id);
    }
}

#[derive(Default)]
struct PickingSync {
    add: Option<[TexId; 2]>,
    badge: Option<TexId>,
    heading: Option<(String, bool)>,
    heading_tex: Option<TexId>,
    names: Vec<String>,
    name_texs: Vec<[TexId; 2]>,
}

fn sync_edit_text(app: &mut App, compositor: &mut Compositor, state: &mut PickingSync) {
    let heading = app.edit_heading();
    if heading != state.heading {
        state.heading = heading.clone();
        let face = heading.map(|(text, large)| {
            let f = slot_ui::heading_text_face(&text, large);
            let (w, h) = (f.w, f.h);
            (upload(compositor, &mut state.heading_tex, f), w, h)
        });
        app.set_heading_face(face);
    }
    let names = app.name_face_names();
    if names != state.names {
        let mut faces = Vec::new();
        for (k, name) in names.iter().enumerate() {
            let pair = [false, true].map(|picked| slot_ui::stack_name_face(name, picked));
            let ids = match state.name_texs.get(k) {
                Some(ids) => {
                    for (id, f) in ids.iter().zip(&pair) {
                        compositor.update_texture(*id, f.w.max(1), f.h.max(1), &f.rgba);
                    }
                    *ids
                }
                None => {
                    let ids = pair
                        .each_ref()
                        .map(|f| compositor.create_texture(f.w.max(1), f.h.max(1), &f.rgba));
                    state.name_texs.push(ids);
                    ids
                }
            };
            faces.push((
                name.clone(),
                [
                    (ids[0], pair[0].w, pair[0].h),
                    (ids[1], pair[1].w, pair[1].h),
                ],
            ));
        }
        state.names = names;
        app.set_name_faces(faces);
    }
}

fn sync_picking(app: &mut App, compositor: &mut Compositor, state: &mut PickingSync) {
    sync_edit_text(app, compositor, state);
    let Some(n) = app.picked_count() else {
        return;
    };
    if app.add_faces_count() != Some(n) {
        let faces = [false, true].map(|light| slot_ui::add_legend(footer_ink(light), n));
        let ids = match state.add {
            Some(ids) => {
                for (id, f) in ids.iter().zip(&faces) {
                    compositor.update_texture(*id, f.w, f.h, &f.rgba);
                }
                ids
            }
            None => faces
                .each_ref()
                .map(|f| compositor.create_texture(f.w, f.h, &f.rgba)),
        };
        state.add = Some(ids);
        app.set_add_faces([(ids[0], faces[0].w), (ids[1], faces[1].w)], n);
    }
    if n > 1 && app.count_badge_count() != Some(n) {
        let f = slot_ui::count_badge_face(n);
        let id = match state.badge {
            Some(id) => {
                compositor.update_texture(id, f.w, f.h, &f.rgba);
                id
            }
            None => compositor.create_texture(f.w, f.h, &f.rgba),
        };
        state.badge = Some(id);
        app.set_count_badge((id, f.w, f.h), n);
    }
}

fn sync_wallpaper(app: &mut App, compositor: &mut Compositor, state: &mut WallpaperSync) {
    let (w, h) = app.wallpaper_size();
    let on_shelf = app.wallpapers_preload();
    if on_shelf {
        sync_shelf_wallpaper(app, compositor, state);
    }
    let shown = match on_shelf {
        true => None,
        false => app.shown_wallpaper().map(|name| (name, (w, h))),
    };
    if !on_shelf && state.image != shown {
        state.image = shown.clone();
        if let Some((name, _)) = &shown {
            if let Some(rgba) = app
                .root()
                .map(|root| root.join("Wallpapers").join(name))
                .and_then(|path| wallpaper_face(&path, w, h))
            {
                let id = match state.tex {
                    Some(id) => {
                        compositor.update_texture(id, w, h, &rgba);
                        id
                    }
                    None => {
                        let id = compositor.create_texture(w, h, &rgba);
                        state.tex = Some(id);
                        id
                    }
                };
                app.set_wallpaper(id);
            }
        }
    }
    if app.quick_menu().is_none() {
        return;
    }
    let label = app.resolved_wallpaper().label();
    if state.label == label {
        return;
    }
    let (dim, lit) = (
        quick_value_face(&label, false),
        quick_value_face(&label, true),
    );
    let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
    let dim = upload(compositor, &mut state.dim, dim);
    let lit = upload(compositor, &mut state.lit, lit);
    app.set_quick_wallpaper_faces((dim, dim_size.0, dim_size.1), (lit, lit_size.0, lit_size.1));
    state.label = label;
}

fn sync_quick_clock(app: &mut App, compositor: &mut Compositor, state: &mut QuickClock) {
    if app.quick_menu().is_none() {
        return;
    }
    let text = date_time_text(app.wall_secs(), app.clock_24h());
    if text == state.shown {
        return;
    }
    let (dim, lit) = (
        quick_value_face(&text, false),
        quick_value_face(&text, true),
    );
    let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
    let dim = upload(compositor, &mut state.dim, dim);
    let lit = upload(compositor, &mut state.lit, lit);
    app.set_quick_clock_faces((dim, dim_size.0, dim_size.1), (lit, lit_size.0, lit_size.1));
    state.shown = text;
}

fn sync_quick_network(app: &mut App, compositor: &mut Compositor, state: &mut QuickClock) {
    if app.quick_menu().is_none() {
        return;
    }
    let text = app.networks_value();
    if text == state.shown {
        return;
    }
    let (dim, lit) = (
        quick_value_face(&text, false),
        quick_value_face(&text, true),
    );
    let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
    let dim = upload(compositor, &mut state.dim, dim);
    let lit = upload(compositor, &mut state.lit, lit);
    app.set_quick_network_faces((dim, dim_size.0, dim_size.1), (lit, lit_size.0, lit_size.1));
    state.shown = text;
}

fn sync_quick_kid_shelf(app: &mut App, compositor: &mut Compositor, state: &mut QuickClock) {
    if app.quick_menu().is_none() {
        return;
    }
    let text = app.kid_shelf_text();
    if text == state.shown {
        return;
    }
    let (dim, lit) = (
        quick_value_face(&text, false),
        quick_value_face(&text, true),
    );
    let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
    let dim = upload(compositor, &mut state.dim, dim);
    let lit = upload(compositor, &mut state.lit, lit);
    app.set_quick_kid_shelf_faces((dim, dim_size.0, dim_size.1), (lit, lit_size.0, lit_size.1));
    state.shown = text;
}

fn sync_quick_romm(app: &mut App, compositor: &mut Compositor, state: &mut [QuickClock; 4]) {
    if app.quick_menu().is_none() {
        return;
    }
    let texts = app.romm_texts();
    if texts.iter().zip(state.iter()).all(|(t, s)| *t == s.shown) {
        return;
    }
    let mut faces = [None; 4];
    for ((text, slot), face) in texts.into_iter().zip(state.iter_mut()).zip(faces.iter_mut()) {
        if text.is_empty() {
            slot.shown = text;
            continue;
        }
        let (dim, lit) = (
            quick_value_face(&text, false),
            quick_value_face(&text, true),
        );
        let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
        let dim = upload(compositor, &mut slot.dim, dim);
        let lit = upload(compositor, &mut slot.lit, lit);
        *face = Some([
            (dim, dim_size.0, dim_size.1),
            (lit, lit_size.0, lit_size.1),
        ]);
        slot.shown = text;
    }
    app.set_quick_romm_faces(faces);
}

fn sync_quick_palette(app: &mut App, compositor: &mut Compositor, state: &mut QuickPalette) {
    if app.in_game_menu().is_none() && app.quick_menu().is_none() {
        return;
    }
    let text = app.gb_colors_label();
    if text == state.shown {
        return;
    }
    let (dim, lit) = (quick_value_face(text, false), quick_value_face(text, true));
    let (dim_size, lit_size) = ((dim.w, dim.h), (lit.w, lit.h));
    let dim = upload(compositor, &mut state.dim, dim);
    let lit = upload(compositor, &mut state.lit, lit);
    app.set_quick_palette_faces((dim, dim_size.0, dim_size.1), (lit, lit_size.0, lit_size.1));
    state.shown = text;
}

const BUNDLED_LOGO: &[u8] = include_bytes!("../../../card/System/bootlogo.bmp");

fn bundled_logo() -> (CartFace, [f32; 4]) {
    let Some(bmp) = crate::bootlogo::decode(BUNDLED_LOGO) else {
        let face = CartFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
        return (face, [0.0, 0.0, 0.0, 1.0]);
    };
    let ground = bmp.rgb.first().copied().unwrap_or([0, 0, 0]);
    let rgba = bmp
        .rgb
        .iter()
        .flat_map(|&px| {
            let [r, g, b] = px;
            [r, g, b, if px == ground { 0 } else { 0xff }]
        })
        .collect();
    let face = CartFace {
        rgba,
        w: bmp.width as u32,
        h: bmp.height as u32,
    };
    let [r, g, b] = ground.map(|c| c as f32 / 255.0);
    (face, [r, g, b, 1.0])
}

fn sync_update(app: &mut App, compositor: &mut Compositor, state: &mut UpdateText) {
    if app.update_shown() && !state.scene {
        state.scene = true;
        let (logo, logo_ground) = bundled_logo();
        let (lw, lh) = (logo.w, logo.h);
        let mut up = |f: CartFace| compositor.create_texture(f.w, f.h, &f.rgba);
        let mut cart = |face: fn() -> CartFace, kind: crate::update::Kind| UpdateCart {
            cart: up(padded(&face(), TURN_PAD)),
            interior: up(slot_ui::cart_interior(&crate::app::cart_for(kind))),
            face: up(face()),
        };
        let carts = [
            cart(update_cart_face, crate::update::Kind::Slot),
            cart(slot_ui::baseos_cart_face, crate::update::Kind::BaseOs),
        ];
        app.set_update_scene(UpdateScene {
            carts,
            chip: up(chip_face(None)),
            board: up(bare_board_face()),
            logo: (up(logo), lw, lh),
            logo_ground,
        });
    }
    let versions = app.update_versions();
    if versions.is_some() && versions != state.versions {
        if let Some((old, new)) = &versions {
            let faces = [
                socket_face_text(old),
                socket_face_text(new),
                chip_face_text(old),
                chip_face_text(new),
            ];
            let mut texes = Vec::new();
            for (face, slot) in faces.into_iter().zip(state.chips.iter_mut()) {
                texes.push(upload_rgba(compositor, slot, face.w, face.h, &face.rgba));
            }
            app.set_update_chips(texes[..2].to_vec(), texes[2..].to_vec());
        }
        state.versions = versions;
    }
    let text = app.update_text();
    if text == state.shown {
        return;
    }
    state.shown = text.clone();
    let Some((title, detail)) = text else {
        app.set_update_faces(Vec::new());
        return;
    };
    let title = match app.update_headline() {
        true => slot_ui::headline_face(&title),
        false => menu_face(&title),
    };
    let (tw, th) = (title.w, title.h);
    let mut faces = vec![(upload(compositor, &mut state.title, title), tw, th)];
    if !detail.is_empty() {
        let detail = menu_detail_face(&detail);
        let (dw, dh) = (detail.w, detail.h);
        faces.push((upload(compositor, &mut state.detail, detail), dw, dh));
    }
    app.set_update_faces(faces);
}

fn sync_about(app: &mut App, compositor: &mut Compositor, state: &mut AboutFace) {
    if !matches!(app.phase(), Phase::About) {
        return;
    }
    let battery = app.battery().map(|b| b.percent);
    if state.tex.is_some() && state.battery == battery {
        return;
    }
    state.battery = battery;
    let build = Build::current();
    let baseos = crate::baseos::var("BASEOS_VERSION");
    let model = crate::baseos::var("BASEOS_MODEL").unwrap_or_default();
    let fields = StickerFields {
        battery,
        serial: &build.serial(),
        version: build.version,
        baseos: baseos.as_deref(),
        model: &model,
    };
    let face = match crate::baseos::var("BASEOS_TARGET").as_deref() {
        Some("rgdsplus") => slot_ui::ds_sticker_face(&fields),
        _ => sticker_face(&fields),
    };
    let (w, h) = (face.w, face.h);
    let id = upload(compositor, &mut state.tex, face);
    app.set_sticker_face(id, w, h);
}

#[derive(Default)]
struct CoreTextures {
    interior: Option<TexId>,
    board: Option<TexId>,
    lid: Option<TexId>,
}

fn sync_core_picker(
    app: &mut App,
    compositor: &mut Compositor,
    builder: &FaceBuilder,
    asked: &mut Option<(Console, String)>,
    tex: &mut CoreTextures,
    built: &mut Option<(Console, String)>,
) {
    if app.take_core_stale() {
        *asked = None;
        *built = None;
    }
    let highlighted = app.selected_cart().map(|c| (c.platform, c.stem.clone()));
    if let Some(cart) = app.selected_cart().filter(|_| *asked != highlighted) {
        builder.request(cart.clone());
        *asked = highlighted.clone();
    }
    let Some(faces) = builder.take() else {
        return;
    };
    let made = Some((faces.platform, faces.stem.clone()));
    if highlighted != made || *built == highlighted {
        return;
    }
    let interior_id = upload_rgba(
        compositor,
        &mut tex.interior,
        faces.interior.w,
        faces.interior.h,
        &faces.interior.rgba,
    );
    app.set_core_interior_face(interior_id);
    let board_id = upload_rgba(
        compositor,
        &mut tex.board,
        faces.board.w,
        faces.board.h,
        &faces.board.rgba,
    );
    let lid_id = upload_rgba(
        compositor,
        &mut tex.lid,
        faces.lid.w,
        faces.lid.h,
        &faces.lid.rgba,
    );
    app.set_core_board_faces(board_id, lid_id);
    *built = made;
}

#[derive(Default)]
struct BoxArtSync {
    loader: Option<crate::box_art::BoxArtLoader>,
    asked: Option<(slot_store::Platform, String)>,
    tex: [Option<TexId>; 2],
}

fn sync_box_art(app: &mut App, compositor: &mut Compositor, sync: &mut BoxArtSync) {
    if !app.two_panels() {
        return;
    }
    let seated = app.seated_cart().cloned();
    let want = seated.as_ref().map(|c| (c.platform, c.stem.clone()));
    if want != sync.asked {
        sync.asked = want;
        app.set_box_art(None);
        if let (Some(cart), Some(root)) = (seated, app.root().map(|r| r.to_path_buf())) {
            sync.loader
                .get_or_insert_with(crate::box_art::BoxArtLoader::spawn)
                .request(root, cart);
        }
    }
    let Some(art) = sync.loader.as_ref().and_then(|l| l.take()) else {
        return;
    };
    if sync.asked.as_ref() != Some(&(art.platform, art.stem.clone())) {
        return;
    }
    let [box_tex, logo_tex] = &mut sync.tex;
    let mut up = |image: Option<crate::box_art::Image>, slot: &mut Option<TexId>| {
        image.map(|(w, h, rgba)| (upload_rgba(compositor, slot, w, h, &rgba), w, h))
    };
    let boxed = up(art.boxed, box_tex);
    let logo = up(art.logo, logo_tex);
    app.set_box_art(Some(crate::app::BoxArtFaces {
        boxed,
        logo,
        stem: art.stem,
        platform: art.platform,
    }));
}

#[derive(Default)]
struct BoxTextSync {
    shown: Option<(String, String)>,
    title: Option<TexId>,
    system: Option<TexId>,
}

fn sync_box_text(app: &mut App, compositor: &mut Compositor, sync: &mut BoxTextSync) {
    let want = app.box_text();
    if want == sync.shown {
        return;
    }
    sync.shown = want.clone();
    let Some((title, system)) = want else {
        app.set_box_text_faces(slot_ui::Printed::default(), slot_ui::Printed::default());
        return;
    };
    let mut put = |text: &str, slot: &mut Option<TexId>| {
        let face = slot_ui::title_line_face(text);
        let w = face.w;
        match w {
            0 => slot_ui::Printed::default(),
            _ => slot_ui::Printed::new(upload(compositor, slot, face), w),
        }
    };
    let title = put(&title, &mut sync.title);
    let system = put(&system, &mut sync.system);
    app.set_box_text_faces(title, system);
}

fn upload(compositor: &mut Compositor, slot: &mut Option<TexId>, face: slot_ui::UndoFace) -> TexId {
    upload_rgba(compositor, slot, face.w, face.h, &face.rgba)
}

fn upload_rgba(
    compositor: &mut Compositor,
    slot: &mut Option<TexId>,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> TexId {
    match *slot {
        Some(id) => {
            compositor.update_texture(id, w, h, rgba);
            id
        }
        None => {
            let id = compositor.create_texture(w, h, rgba);
            *slot = Some(id);
            id
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_stretch_does_not_change_the_bezel_key() {
        let card = BezelPick::File("Pocket.png".into());
        assert_eq!(bezel_key(&card, Some(BezelArt::Default), None), None);
        assert_eq!(bezel_key(&card, None, None), None);
        assert_eq!(
            bezel_key(&BezelPick::Default, None, Some(BezelArt::Default)),
            Some(BezelArt::Default)
        );
        assert_eq!(
            bezel_key(
                &BezelPick::Default,
                Some(BezelArt::Fit3x2),
                Some(BezelArt::Default)
            ),
            Some(BezelArt::Fit3x2)
        );
    }
}
