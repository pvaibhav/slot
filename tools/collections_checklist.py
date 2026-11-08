#!/usr/bin/env python3
import os
import re
import subprocess
import sys

COL = "slot::collections"
SEL = "slot::collections_select"
REN = "slot::render_collections"
CHK = "slot::collections_checklist"
EDT = "slot::editing"
VID = "slot::video_shelf"
DEVICE = "device"
SHORTCUT = "shortcut"

CHECKLIST = [
    ("Browsing", [
        ("With no collections, L2 / R2 on Favorites do nothing", [f"{COL} a_card_without_collections_behaves_as_before"]),
        ("On Favorites, R2 goes to the first collection, L2 to the last; the stack wraps both ways", [f"{COL} r2_on_favorites_shows_the_next_collection_and_wraps"]),
        ("L1 / R1 from any collection moves along the row as if you were on Favorites", [f"{CHK} l1_r1_from_a_collection_move_as_if_on_favorites"]),
        ("Leaving the stack and coming back with L1 / R1 lands on the collection you were last on", [f"{COL} the_stack_remembers_its_collection_across_shelves_and_reboots"]),
        ("A collection with no games on the card is skipped by L2 / R2", [f"{COL} an_empty_collection_is_skipped_by_l2_r2"]),
        ("With Favorites empty, the first collection takes Favorites' place in the row (Shelf Mode All)", [f"{COL} with_no_favorites_the_first_collection_takes_their_place", f"{COL} unfavoriting_the_last_game_keeps_the_view_on_the_stack"]),
        ("Every shelf change is an instant cut, with no toast for L2 / R2", [f"{CHK} l2_r2_on_the_stack_never_toast", f"{COL} l2_and_r2_do_nothing_off_the_stack"]),
        ("The collection's name shows under the cart, like a system name", DEVICE),
        ("A Wallpapers/Collections/<name>.png shows on that collection with Wallpaper set to Shelf", [
            "slot::wallpaper a_collection_png_shows_on_its_shelf",
            "slot::wallpaper a_pngless_collection_shows_the_normal_wallpaper",
            "slot::wallpaper under_shelf_a_pngless_collection_shows_favorites_png",
        ]),
        ("Settings > Shelf Mode > Collections gives each collection its own spot in the L1 / R1 row; L2 / R2 do nothing there", [
            f"{COL} collections_mode_flattens_collections_into_the_row",
            f"{COL} l2_r2_do_nothing_in_collections_mode",
            f"{COL} collections_mode_hides_an_empty_favorites",
            f"{COL} the_shelf_mode_row_reads_and_cycles",
        ]),
        ("Switching Shelf Mode while on a system shelf lands you on a shelf that still shows", [
            f"{COL} changing_shelf_mode_keeps_the_shelf_when_it_still_exists",
            f"{COL} leaving_collections_mode_on_favorites_keeps_the_stack_on_favorites",
            f"{COL} collections_mode_never_boots_onto_a_hidden_system_shelf",
        ]),
        ("Shelf Mode survives a restart", ["slot-store::shelf_mode shelf_mode_survives_a_restart"]),
        ("Restarting on a collection boots back onto it, on the same game", [f"{COL} a_reboot_on_a_collection_comes_back_to_it", f"{COL} a_collection_shelf_keeps_its_file_order_across_systems"]),
        ("Game Boy Advance Video is no longer in the L1 / R1 row", [f"{VID} video_is_not_in_the_l1_r1_row"]),
        ("On the GBA shelf, L2 / R2 flip to GBA Video and back", [f"{VID} r2_on_gba_flips_to_video_and_back"]),
        ("L1 / R1 from GBA Video moves along the row from GBA's spot", [f"{VID} l1_r1_from_video_moves_along_the_row_from_gba"]),
        ("With no GBA games, GBA Video takes GBA's spot in the row", [f"{VID} with_no_gba_games_video_takes_the_gba_seat"]),
    ]),
    ("Adding one game", [
        ("A quick tap of Y still favorites, with no jiggle", [f"{CHK} a_y_tap_favorites_and_leaves_no_hold_behind", f"{COL} y_elsewhere_still_favorites"]),
        ("Holding Y jiggles the cart from about 150 ms and the neighbors dim", [
            f"{EDT} motion::motion_hold_jiggle_waits_150_ms_then_reaches_2_2",
            f"{EDT} motion::motion_neighbors_dim_to_75_percent",
            f"{REN} transition_the_hold_jiggles_the_cart",
        ]),
        ("The add screen opens at 485 ms; letting go early only favorites", ["slot-input::gesture a_y_hold_fires_once_at_485_and_never_favorites", f"{SEL} holding_y_opens_the_add_screen_with_that_game"]),
        ("Entry: neighbors slide off left and right, the cart shrinks up to float, the stacks fade up", [f"{REN} transition_entry_mid_frame_has_neighbours_leaving"]),
        ("Heading reads \"Add to which collection?\"", [f"{REN} the_add_screen_draws_stacks_and_the_heading"]),
        ("No collections: only New Collection shows", [f"{CHK} with_no_collections_only_new_collection_is_offered"]),
        ("One collection: it and New side by side", [f"{CHK} with_one_collection_it_sits_beside_new_collection", f"{EDT} layouts_follow_the_choice_count"]),
        ("Two collections: both, with New in the middle", [f"{EDT} with_two_collections_new_collection_sits_in_the_middle"]),
        ("Three or more: a looping carousel, sliding under the cart", [f"{REN} transition_the_carousel_slides_under_the_cart", f"{EDT} picking_wraps_both_ways"]),
        ("Favorites is never offered", [f"{CHK} favorites_is_never_offered"]),
        ("A collection that already holds the game is not offered", [f"{EDT} a_collection_holding_the_game_is_not_offered"]),
        ("Each collection shows as a stack of its own first three carts, with its name under it that never fades", [
            f"{EDT} a_stack_is_the_first_three_games_on_the_card",
            f"{SEL} the_add_screen_asks_for_the_faces_in_every_stack",
            f"{REN} the_picked_name_stays",
        ]),
        ("Left / Right glides the cart to the picked stack", [f"{SEL} left_and_right_pick_and_wrap"]),
        ("It always opens on New Collection", [f"{SEL} the_screen_opens_on_new_collection_after_adding_to_one", f"{EDT} the_screen_opens_on_new_collection"]),
        ("A: the cart slides into the front of the stack and nothing else moves", [f"{SEL} the_add_exit_shows_the_stack_as_it_was_before_adding"]),
        ("Then the stacks vanish, the cart grows home on an empty shelf, the neighbors slide in, and \"Added to <name>\" shows", [f"{REN} transition_add_exit_grows_home_on_an_empty_shelf", f"{SEL} a_adds_and_returns_to_the_cart_you_were_on"]),
        ("You end where you started, on the same game", [f"{SEL} a_adds_and_returns_to_the_cart_you_were_on"]),
        ("B: the stacks vanish fast and the cart settles home with no add", [f"{SEL} b_with_one_game_returns_to_the_shelf"]),
        ("Hold Y on a collection shelf opens Remove instead", [f"{COL} holding_y_on_a_collection_opens_remove"]),
        ("Collections mode: adding from a system shelf while selecting lands on the collection", [f"{SEL} collections_mode_from_a_system_shelf_lands_on_the_destination"]),
    ]),
    ("Multi-select", [
        ("Holding SELECT for 600 ms enters selecting, with the cart in front already ticked", [f"{SEL} holding_select_starts_with_the_cart_in_front_checked"]),
        ("A quick tap of SELECT still opens the cart studio", ["slot::studio_ui tap_opens_the_studio_for_the_selected_cart"]),
        ("Ticked carts jiggle softly (1.6 degrees), unticked ones are still", [f"{EDT} motion::motion_soft_jiggle_stays_within_1_6", f"{REN} ticked_carts_show_no_box"]),
        ("The A legend reads Unselect on a ticked cart and Select on an unticked one", [f"{REN} the_a_legend_says_select_or_unselect"]),
        ("Footer: B Cancel, A Select / Unselect, START Add n; the START legend dims with nothing ticked", DEVICE),
        ("The slot is plugged while selecting", [f"{REN} selecting_plugs_the_slot"]),
        ("Ticks survive L1 / R1 to other shelves; system shelves show even in Collections mode", [f"{SEL} checks_survive_changing_shelves", f"{SEL} collections_mode_shows_platforms_while_picking"]),
        ("B cancels and clears every tick", [f"{SEL} b_while_selecting_clears_and_ends"]),
        ("B on a system shelf in Collections mode does nothing", [f"{SEL} collections_mode_shows_platforms_while_picking"]),
        ("START with nothing ticked does nothing", [f"{SEL} start_with_nothing_checked_does_nothing", f"{SEL} start_while_selecting_never_opens_the_core_picker"]),
        ("START opens the add screen with the ticked carts as a fanned bundle and a count badge", [f"{SEL} the_bundle_asks_for_the_faces_of_what_it_carries", f"{SEL} a_multi_add_exit_never_flies_one_cart_home"]),
        ("A collection that holds every ticked game is not offered; one holding some is, and only the missing games are added", [
            f"{SEL} a_collection_holding_every_ticked_game_is_not_offered",
            f"{EDT} a_collection_holding_some_ticked_games_is_offered",
            f"{SEL} three_ticked_with_one_already_in_adds_two",
        ]),
        ("A adds them; the toast reads \"Added n to <name>\"", [f"{SEL} three_ticked_with_one_already_in_adds_two"]),
        ("B from the add screen returns to where START was pressed, everything still ticked and jiggling", [f"{SEL} b_in_multi_returns_to_selecting_where_start_was_pressed_with_ticks_kept"]),
        ("Known shortcut: the ticked carts appear as a bundle instead of each flying in, and B has no reverse animation", SHORTCUT),
    ]),
    ("New Collection", [
        ("A on New Collection opens the keyboard titled New Collection", [f"{CHK} the_keyboard_is_titled_new_collection"]),
        ("Y on the keyboard cancels back to the add screen, still on New", [f"{SEL} cancelling_the_keyboard_returns_to_the_add_screen"]),
        ("START with a good name creates it and adds the game", [f"{SEL} new_collection_names_and_adds", f"{CHK} spaces_around_a_name_are_trimmed"]),
        ("The keyboard dissolves into the add screen, the box is labelled with the typed name, and the cart drops into it as the dashed outline fades", [f"{REN} a_new_collection_shows_its_name_as_the_cart_drops_in"]),
        ("Then the normal exit runs and the toast reads \"Added to <name>\"", [f"{SEL} new_collection_names_and_adds"]),
        ("Same with no collections and with New among other collections", [f"{CHK} a_first_collection_names_and_adds", f"{SEL} new_collection_names_and_adds"]),
        ("A name already taken (any letter case) shows \"Name already exists, use a different name\" in bright 18 px text", [
            f"{SEL} a_name_already_taken_says_so",
            f"{CHK} a_taken_name_in_another_case_is_refused",
            "slot-ui::keyboard a_refusal_reads_brighter_and_larger_than_a_hint",
        ]),
        ("A blank name, \"order\", a leading dot or a trailing dot shows \"Name not allowed, use a different name\"", [
            f"{CHK} every_name_that_cannot_be_a_file_is_refused",
            f"{SEL} a_bad_name_keeps_the_keyboard_open",
            "slot-store::collections names_that_cannot_be_files_are_refused",
        ]),
        ("After a refusal the selected key, the text and Shift stay where they were", [f"{CHK} a_refusal_keeps_the_text_the_key_and_shift"]),
        ("The new collection goes to the end of the stack and order.txt", [f"{SEL} new_collection_names_and_adds"]),
        ("Known shortcut: the keyboard's keys vanish on the first frame and only its backdrop fades", SHORTCUT),
    ]),
    ("Removing", [
        ("On a collection, a quick tap of Y still favorites", [f"{COL} tapping_y_on_a_collection_favorites_and_never_removes", f"{SEL} tapping_y_still_favorites_on_a_collection_shelf"]),
        ("Holding Y opens \"Remove from <name>?\" with the cart floating over a tilted stack of the three carts that will surround the gap", [
            f"{COL} holding_y_on_a_collection_opens_remove",
            f"{EDT} the_remove_stack_is_the_carts_around_the_gap",
            f"{REN} the_remove_screen_draws_the_stack_low_and_the_cart_under_the_heading",
        ]),
        ("No name under the stack; footer B Keep, A Remove", DEVICE),
        ("Carts on the way in and out never flash as solid label-colored boxes, on a big library too", [
            f"{COL} keep_holds_on_to_the_faces_the_remove_screen_drew",
            f"{COL} remove_holds_on_to_the_faces_the_remove_screen_drew",
            f"{COL} the_remove_screen_draws_carts_whose_faces_are_already_wanted",
        ]),
        ("A: the cart fades off the top, the stack fans into the shelf, the toast reads \"Removed from <name>\"", [f"{COL} remove_takes_it_out_and_centers_the_next_game"]),
        ("The other carts keep their labels with no flash after the removal", ["slot-ui::shelf restocking_keeps_the_faces_of_carts_that_stay"]),
        ("Removing the last cart in the row centers the first game", [f"{COL} removing_the_last_cart_on_a_shelf_centers_the_first"]),
        ("B (Keep): the cart grows home while the stack fans into its neighbors; the fourth card fades out in about 120 ms", [f"{COL} keep_changes_nothing", f"{EDT} motion::motion_the_card_left_out_of_the_fan_is_gone_a_quarter_of_the_way_in"]),
        ("Last game: the heading reads \"Remove last game from <name>?\" in larger text and the cart settles to 82% in place", [f"{REN} the_last_game_heading_is_larger"]),
        ("Keep on the last game settles from where it was, with no jump", [f"{REN} transition_keep_on_the_last_game_settles_from_where_it_was"]),
        ("Remove on the last game: the cart rises off the top and you land on the next shelf", [
            f"{REN} transition_remove_rises_off_the_top",
            f"{COL} removing_the_last_game_stays_on_the_stack",
            f"{COL} removing_the_last_game_with_no_favorites_moves_to_the_next_collection",
            f"{COL} removing_the_last_game_anywhere_leaves_the_stack_for_the_row",
            f"{COL} removing_the_last_game_in_collections_mode_goes_to_the_next_shelf",
        ]),
        ("Removing the last game deletes <name>.txt and its order.txt line; its wallpaper PNG stays", [f"{COL} removing_the_last_game_deletes_the_collection", f"{CHK} removing_the_last_game_keeps_the_wallpaper"]),
        ("The deleted collection is gone from the add screen", [f"{CHK} a_deleted_collection_is_gone_from_the_add_screen"]),
        ("A collection whose games are all missing from the card stays on the card, hidden", [f"{CHK} a_collection_whose_games_are_all_missing_stays_on_the_card_hidden"]),
    ]),
    ("Moving a game", [
        ("X on a collection picks the game up at once: it jiggles, the heading reads Move in 20 px text, the footer shows B Cancel and A Drop", [f"{COL} x_on_a_collection_picks_up_at_once", f"{REN} moving_draws_the_move_heading"]),
        ("X on Recents still removes the game from Recents", ["slot::recents x_on_recents_takes_the_selected_game_off_and_selects_the_next"]),
        ("A tap of Left / Right moves one spot; the neighbor slides past underneath, dipping and shrinking", [
            f"{COL} a_tap_right_moves_one_spot",
            f"{EDT} motion::motion_a_tap_settles_on_exactly_one_spot",
            f"{EDT} motion::motion_the_passed_neighbour_crosses_under_the_held_one",
        ]),
        ("Holding Left / Right keeps moving at a steady pace with no hitch, the shelf scrolling with it", [
            f"{COL} holding_right_keeps_moving_then_settles_on_the_next_spot",
            f"{EDT} motion::motion_a_hold_never_hitches",
            f"{EDT} motion::motion_the_camera_keeps_the_held_item_one_spot_out_then_catches_up",
        ]),
        ("The shelf re-centers only when you let go, with no snap at the end", [f"{REN} moving_a_game_has_no_jumps"]),
        ("The cart doesn't jiggle while it travels and starts again once it settles", [f"{EDT} motion::motion_the_held_one_is_still_while_it_moves"]),
        ("The move loops past either end like shelf browsing", [f"{COL} moving_past_the_end_wraps_to_the_front"]),
        ("A drops it and saves the new order to <name>.txt", [f"{COL} a_drops_and_writes_the_order"]),
        ("B puts everything back the way it was", [f"{COL} b_puts_it_back"]),
        ("A one-game collection does nothing on Left / Right, with no wobble", [f"{COL} holding_a_direction_in_a_one_game_collection_never_moves_the_cart"]),
    ]),
    ("Moving a collection", [
        ("Holding X past 450 ms gathers the shelf into the collection's stack, lifted, heading \"Move <name>\"", [
            f"{COL} holding_x_moves_the_collection",
            f"{COL} releasing_x_keeps_the_collection_move_open",
            f"{REN} transition_gather_frames_converge",
        ]),
        ("A game move started by the tap is undone when the hold turns into a collection move", [f"{CHK} the_tapped_game_move_is_dropped_when_the_hold_lifts_the_collection"]),
        ("Other collections sit beside it on one base line, like carts on a shelf, names centered under them", [f"{REN} the_collection_row_names_sit_under_their_stacks"]),
        ("The lifted stack does not jiggle", [f"{REN} a_lifted_collection_does_not_jiggle"]),
        ("Left / Right swaps it with its neighbor, which slides past underneath", [f"{REN} moving_a_collection_has_no_jumps"]),
        ("Holding Left / Right keeps going, looping around, with no jump at the end", [f"{REN} moving_a_collection_has_no_jumps", f"{COL} an_empty_collection_keeps_its_place_across_the_wrap"]),
        ("A drops it and saves order.txt; hidden collections keep their places", [
            f"{COL} a_rewrites_order_and_stays_on_the_moved_collection",
            f"{EDT} hidden_collections_keep_their_place_after_a_move",
            f"{EDT} a_hidden_collection_at_the_front_stays_at_the_front",
        ]),
        ("B cancels and the order is unchanged", [f"{COL} b_restores_the_collection_order"]),
        ("Shelf Mode Collections: the L1 / R1 row follows the new order", [f"{COL} collections_mode_row_follows_the_new_order"]),
        ("Known shortcut: the gather is a cross-fade, and dropping doesn't fan back out", SHORTCUT),
    ]),
    ("Files, settings and Gamebit", [
        ("Collections live in Collections/<name>.txt, one gba/Stem line per game, plus Collections/order.txt", [
            "slot-store::collections writing_a_collection_round_trips",
            "slot-store::collections lines_keep_their_file_order_across_platforms",
            "slot-store::collections collections_come_back_in_order_txt_order",
        ]),
        ("A hand-made .txt dropped on the card shows up at boot, added to the end of order.txt", [
            "slot-store::collections a_file_missing_from_order_txt_joins_the_end_and_is_written_in",
            "slot-store::collections hand_edited_files_load_cleanly",
        ]),
        ("A line in order.txt with no file is dropped; a game missing from the card is left off the shelf and kept in the file", [
            "slot-store::collections an_order_line_without_a_file_is_dropped",
            f"{COL} a_missing_rom_is_hidden_and_its_line_kept",
        ]),
        ("Settings > Controls has a Collections page after Shelf, with Previous and Next; rebinding them works and survives a restart", [
            "slot-ui::quick_menu collections_sits_after_shelf_in_controls",
            "slot::bindings collection_defaults_reach_the_shelf_as_actions",
            "slot::bindings reset_all_on_the_collections_page_leaves_the_shelf_page_alone",
            "slot-store::bindings collection_rows_round_trip",
        ]),
        ("Renaming or deleting a game in Gamebit updates or drops it in every collection", ["slot::server_games rename_carries_collections", "slot::server_games purge_removes_from_collections"]),
        ("Closing Gamebit reloads collections it edited", ["slot::server_ui close_reloads_the_collections_the_server_edited"]),
        ("Hold Y still jiggles when Favorite is rebound to another button (known minor)", SHORTCUT),
    ]),
    ("Regressions to glance at", [
        ("Normal shelf browsing, scrolling and L1 / R1 feel unchanged", DEVICE),
        ("Favoriting and unfavoriting on the Favorites shelf don't flash labels", ["slot-ui::shelf restocking_keeps_the_faces_of_carts_that_stay"]),
        ("X on Recents, START for the core picker and a SELECT tap for the cart studio still work", [
            "slot::recents x_on_recents_takes_the_selected_game_off_and_selects_the_next",
            "slot::app start_on_the_shelf_opens_the_core_picker_on_the_carts_current_core",
            "slot::studio_ui tap_opens_the_studio_for_the_selected_cart",
        ]),
        ("Settings > General rows are in the expected order", DEVICE),
        ("No frame drops on the add, remove and move screens on the RG35XXSP as well as the RG SP", DEVICE),
    ]),
]

STATUS = re.compile(r"^\s*(PASS|FAIL|TIMEOUT|SIGSEGV|SIGABRT|SIGKILL|LEAK|FLAKY)\b.*?\]\s+(?:\(\s*\d+/\d+\)\s+)?(\S+)\s+(\S+)\s*$")


def wanted():
    seen = []
    for _, items in CHECKLIST:
        for _, tests in items:
            if isinstance(tests, list):
                for t in tests:
                    if t not in seen:
                        seen.append(t)
    return seen


def filterset(tests):
    parts = []
    for t in tests:
        binary, name = t.split(" ", 1)
        parts.append(f"(binary_id(={binary}) & test(={name}))")
    return " | ".join(parts)


def run(tests):
    env = dict(os.environ, SLOT_SILENT="1")
    packages = sorted({t.split("::")[0] for t in tests})
    binaries = sorted({t.split(" ")[0].split("::")[1] for t in tests})
    cmd = ["cargo", "nextest", "run", "--no-fail-fast"]
    for p in packages:
        cmd += ["-p", p]
    for b in binaries:
        cmd += ["--test", b]
    cmd += [ "--status-level", "all", "--final-status-level", "none", "-E", filterset(tests)]
    proc = subprocess.run(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    results = {}
    for line in proc.stdout.splitlines():
        m = STATUS.match(line)
        if m:
            status, binary, name = m.groups()
            results[f"{binary} {name}"] = "PASS" if status in ("PASS", "FLAKY") else status
    return proc, results


def main():
    tests = wanted()
    if "--list" in sys.argv:
        print("\n".join(tests))
        return 0
    proc, results = run(tests)
    missing = [t for t in tests if t not in results]
    failed = [t for t in tests if results.get(t, "PASS") != "PASS"]
    if missing and not results:
        sys.stdout.write(proc.stdout)
        return 1
    counts = {"pass": 0, "fail": 0, "device": 0, "shortcut": 0}
    for section, items in CHECKLIST:
        print(f"\n## {section}\n")
        for text, check in items:
            if check == DEVICE:
                counts["device"] += 1
                print(f"- [ ] {text}  (on device)")
                continue
            if check == SHORTCUT:
                counts["shortcut"] += 1
                print(f"- [-] {text}")
                continue
            bad = [t for t in check if results.get(t) != "PASS"]
            if bad:
                counts["fail"] += 1
                print(f"- [ ] {text}")
                for t in bad:
                    print(f"      {results.get(t, 'NOT RUN')}: {t}")
            else:
                counts["pass"] += 1
                print(f"- [x] {text}")
    print(
        f"\n{counts['pass']} items pass, {counts['fail']} fail, "
        f"{counts['device']} need the device, {counts['shortcut']} are known shortcuts"
    )
    print(f"{len(tests)} tests named, {len(results)} ran")
    if failed or missing:
        log = os.path.join(os.environ.get("TMPDIR", "/tmp"), "collections_checklist.log")
        with open(log, "w") as f:
            f.write(proc.stdout)
        print(f"full nextest output: {log}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
