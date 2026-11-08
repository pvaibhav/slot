mod art;
pub use art::{cover, cover_bytes, decode_png_bytes};
mod bitmaps;
pub use bitmaps::{
    link_port, link_port_light, slot_back, slot_back_light, slot_front, slot_front_light,
    slot_plugged, slot_plugged_light, Mould, SLOT_H, SLOT_W,
};
mod backdrop;
mod barcode;
mod battery;
mod board;
mod cart;
mod checkbox;
mod edit_text;
mod clock;
mod draw;
mod footer;
mod guide;
mod hud;
mod icon;
mod keyboard;
mod link_art;
mod network_menu;
mod plate;
mod polaroids;
mod power_menu;
mod quick_menu;
mod rebuild;
mod receipt;
mod refusal;
mod shelf;
mod shell;
mod shot_viewer;
mod silhouette;
mod slot_chrome;
#[cfg(feature = "nds")]
mod box_screen;
#[cfg(feature = "nds")]
pub use box_screen::BoxScreen;
mod sticker;
pub mod text;
pub mod house_label;
pub mod hue;
pub mod liner;
pub mod palette;
pub mod sticker_curl;
mod toast;

pub use art::decode as decode_png;
pub use backdrop::{
    bezel_face, blur_background, blur_background_at, default_background, default_background_at,
    default_game_bezel_face, draw_backdrop, reference_ground_covers,
    draw_ground, game_bezel_face, wallpaper_face, BezelArt,
};
pub use barcode::{code39, CODE39_NARROW, CODE39_WIDE};
pub use battery::{draw_gauge, BOLT_PX, GAUGE_H, GAUGE_W, WALL};
pub use board::{
    bare_board_face, baseos_cart_face, board_at, board_face, board_from, board_from_shaped,
    board_in, board_x, board_zoom, chip_face, chip_face_text, chip_shadow_face, gb_chip_face,
    gb_socket_face, grown, lid_at,
    lid_from, lid_from_shaped, lift_of, on_board, padded, rom_marking, rom_marking_face, seats,
    shelf_cart, shelf_cart_at, shelf_cart_shaped, slide_of, socket_face, socket_face_text,
    update_cart_face, Placed, Seats, BOARD_H, BOARD_W, BOARD_Y, CHIP_H, CHIP_TIP, CHIP_U, CHIP_V,
    CHIP_W, FINGERS, FINGER_H, FINGER_Y, GBA_SEATS, HOP_LIFT, LID_TURN, ROM_H, ROM_W, ROM_X, ROM_Y,
    SHADOW_H, SHADOW_W, SLIDE_SHARE, SLIDE_UP, SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W, TURN_PAD,
};
pub use cart::{
    bare_face, cart_backing, cart_box, cart_face, cart_face_with, cart_interior, cart_shadow, clean_label,
    folder_gb_shell, gb_cart_shadow, gb_shell_of, is_clear, label_colour, label_panel, label_size,
    label_tags, label_text, neutral_shell, seated_box, shelf_shape, shows_interior, text_label, CartFace, CART_H, CART_W,
    GBC_LABEL_H, GBC_LABEL_W, GBC_LABEL_X, GBC_LABEL_Y, GB_CART_H, GB_CART_W, GB_LABEL_H,
    GB_LABEL_W, GB_LABEL_X, GB_LABEL_Y, LABEL_H, LABEL_W, LABEL_WELL, LABEL_X, LABEL_Y,
};
#[cfg(feature = "nds")]
pub use cart::{DS_LABEL_H, DS_LABEL_W, DS_LABEL_X, DS_LABEL_Y};
#[cfg(feature = "nds")]
pub use bitmaps::{ds_slot_back, ds_slot_back_light, ds_slot_front, ds_slot_front_light, DS_CART_H, DS_CART_W, DS_MOUTH_W};
pub use clock::{
    clock_label, date_time_text, hhmm, time_text, ClockFaceSet, ClockFaces, ClockPicker, ClockRow,
    ClockScreen, Part,
};
pub use draw::{Canvas, Draw, TexId, OUT_H, OUT_W};
pub use footer::{
    draw_corners, draw_footer, draw_kid_icon, draw_wifi_icon, footer_y, Printed, FOOTER_Y, KID_ICON_PX,
};
pub use guide::{
    cell, fitting_zoom, guide_help_face, mono_font, page_box, Guide, GuideGlyphs, GuideView,
    GUIDE_COLUMNS, GUIDE_HELP_HINT, GUIDE_HELP_TITLE, GUIDE_HINTS, GUIDE_ZOOMS, PAN_COLS,
};
pub use keyboard::{
    draw_keyboard, draw_keyboard_legends, key_rect, keyboard_cap_faces, keyboard_faces,
    keyboard_help_columns, keyboard_help_face, keyboard_text_face, Cap, KeyTex, Keyboard,
    KeyboardFaces, KeyboardTex, Layout, Shortcut, Special, Typed, KEYBOARD_HELP_TITLE,
};
pub use network_menu::{
    bars_rect, lock_rect, net_bars_face, net_lock_face, net_row_top, net_window, pair_face,
    pair_text_width, wifi_face, PAIR_QR, NetFace, NetIcons, NetLine, NetworkList, PairScreen, PairSlot,
};
pub use hud::{
    badge_at, ff_badge, footer_ink, FfState, Hud, HudKind, LinkBadge, Millis, ALARM_BAND_H,
    HUD_ICON_PX, HUD_INK, HUD_MS, LINK_HOST_INK, LINK_JOIN_INK, PLATE_H,
};
pub use checkbox::count_badge_face;
pub use edit_text::{heading_text_face, stack_name_face};
pub use icon::{badge_face, heart_face, icon_box, icon_face, warm_fonts, Badge, Icon};
pub use link_art::{
    link_art, LinkArt, ADAPTER_BASE_X, ADAPTER_BASE_Y, ADAPTER_H, ADAPTER_W, ARCS, ARROW_H,
    ARROW_LEFT_X, ARROW_RIGHT_X, ARROW_W, ARROW_Y, CLICKS_H, CLICKS_W, CLICKS_X, CLICKS_Y, PLUG_H,
    PLUG_TIP_X, PLUG_W, PORT_H, PORT_W, PORT_Y,
};
pub use receipt::{receipt_blank, receipt_face, ReceiptText};
pub use plate::{
    arrows_hint_coloured, arrows_hint_face, arrows_hint_width, cap_width,
    hint_coloured, hint_face, hint_quad, hint_row, hint_width, pad_hint_face, pad_hint_width,
    tick_cap_face, title_face, title_line_face, word_face, word_ink, word_width, Hint, UndoFace,
    ARROW_GAP, CAP, CAP_GAP, HINT_EDGE, HINT_GAP, HINT_H, TITLE_H, TITLE_LINE_H,
    TITLE_W,
};
pub use polaroids::{
    draw_album_chrome, photo_face, AlbumChrome, PhotoFace, Polaroids, DOT, LEGEND, PHOTO_H, PHOTO_W,
};
pub use power_menu::{heading_face, menu_detail_face, menu_face, PowerChoice, MENU_PAD};
pub use quick_menu::{
    add_legend, clock_legend_faces, core_button_legends, picking_legend_faces, delete_legends, draw_menu_ground, link_button_legends,
    fetch_choice_legends, fetch_legends, fetch_skip_legend, print_stop_legend, prompt_legends,
    studio_legend,
    StudioGlyph, quick_description_face, quick_label_face, quick_legend_faces,
    quick_pointer_face, quick_row_top, quick_row_top_at, quick_scroll, quick_value_face,
    quick_warning_value_face, quick_window, repair_legends, update_legends, visible_rows, QuickMenu,
    QuickMenuFaces, QuickRow, QuickScroll, QuickValue, QUICK_EDGE, QUICK_LABEL_X, QUICK_PITCH,
    QUICK_VISIBLE,
};
pub use rebuild::{headline_face, rebuild_count_face, rebuild_title_face, RebuildScreen};
pub use refusal::Refusal;
pub use shelf::{box_size, foot_y, turn_about, repeat_gap, rest_y, Shelf, HEART_INK, HEART_SCALE, REPEAT_DELAY_MS};
pub use shell::{
    gb_plastics, gb_table_shells, gba_plastics, gba_shell_for,
    indexed_shell, lookup_order_is_exact_then_family_then_default, shell_for, shell_presets, table_keys, Finish,
    Shell, DEFAULT_SHELL, DMG_SHELL, DUAL_MODE_SHELL, GB_CLEAR_SHELL,
};
pub use shot_viewer::{
    grid_cell, grid_rows_shown, thumbnail, ShotMode, ShotViewer, GRID_COLS, GRID_GAP, MAX_ZOOM,
    PAN_KEY, PAN_TAP, SHOT_HINTS,
};
pub use silhouette::{gb_silhouette, silhouette, GbShell};
pub use slot_chrome::{
    draw_empty_slot, draw_plugged_slot, draw_slot_back, draw_slot_front, draw_slot_title, ease,
    band_y, edge, housing, opening, recess, seated_rect, slot_name_y, SlotChrome, ALERT_PX, LIP_H, MOUTH_H,
    MOUTH_W,
};
pub use sticker::{
    draw_sticker, ds_head_rows, ds_sticker_face, ds_sticker_lines, head_rows, sticker_face,
    sticker_lines, sticker_rect, sticker_rect_sized, StickerFields, COPYRIGHT, CREDITS, DC,
    DS_CREDITS, DS_STICKER_H, DS_STICKER_W, HOME, ORIGIN, STICKER_H, STICKER_W,
};
pub use toast::{toast_box, toast_face, toast_face_text, toast_rect, Toast};
