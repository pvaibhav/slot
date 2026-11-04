use slot::print_scene::{Cue, PrintScene};

const PRINT_AT: u64 = 260 + 640 + 260;
const RIP_AT: u64 = PRINT_AT + 3200 + 350;
const CARRY_AT: u64 = RIP_AT + 960;
const GAP_AT: u64 = CARRY_AT + 340;
const RETURN_AT: u64 = GAP_AT + 140;
const REST_AT: u64 = RETURN_AT + 560;
const CUT: f32 = 407.0;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn timeline_matches_the_table() {
    let s = PrintScene::new(1000);
    let at = |t: u64| s.frame(1000 + t);
    assert!(near(at(0).slide, 1.0));
    assert!(near(at(130).slide, 0.5));
    assert!(near(at(130).printer_y, 490.0));
    assert!(near(at(260).slide, 0.0));
    assert!(near(at(260).printer_y, 490.0));
    assert!(near(at(260 + 320).printer_y, 490.0 * 0.125));
    assert!(near(at(900).printer_y, 0.0));
    assert!(!at(899).lamp_on);
    assert!(!at(900 + 117).lamp_on);
    assert!(at(900 + 118).lamp_on);
    assert!(at(PRINT_AT - 1).paper.is_none());
    let p = at(PRINT_AT).paper.unwrap();
    assert_eq!(p.pivot, (331.0, 727.0));
    assert!(near(p.lift, 4.0));
    assert!(near(p.tear, -1.0));
    assert!(near(p.fold, -60.0));
    assert!(!p.bow);
    assert_eq!(at(RIP_AT - 1).paper.unwrap().pivot, (331.0, 302.0));
    assert!(near(at(RIP_AT).paper.unwrap().fold, -40.0));
    assert!((at(CARRY_AT - 1).paper.unwrap().fold - 500.0).abs() < 1.0);
    let c = at(CARRY_AT).paper.unwrap();
    assert_eq!(c.pivot, (331.0, 302.0));
    assert!(near(c.tear, 400.0));
    let e = 170.0 / 340.0;
    let m = e * (0.55 + 0.45 * e);
    let c = at(CARRY_AT + 170).paper.unwrap();
    assert!(near(c.pivot.0, 331.0 + 170.0 * m));
    assert!(near(c.pivot.1, 302.0 + 640.0 * m));
    assert!(near(c.scale, 1.03));
    assert!(near(at(RETURN_AT).paper.unwrap().pivot.1, 960.0));
    assert_eq!(at(RETURN_AT).paper.unwrap().pivot.0, 668.0);
    let r = at(REST_AT).paper.unwrap();
    assert_eq!(r.pivot, (668.0, 462.0));
    assert!(r.bow);
    assert!(near(r.lift, 9.0));
    assert!(near(r.fold, -60.0));
    assert!(near(r.tear, 400.0));
}

#[test]
fn print_moves_in_62_steps() {
    let s = PrintScene::new(0);
    let mut seen = Vec::new();
    for t in PRINT_AT..RIP_AT - 350 {
        let y = s.frame(t).paper.unwrap().pivot.1;
        if seen.last() != Some(&y) {
            seen.push(y);
        }
    }
    assert_eq!(seen.len(), 62);
    for w in seen.windows(2) {
        assert!(near(w[0] - w[1], 425.0 / 62.0));
    }
}

#[test]
fn paper_stays_behind_blades_until_the_rip_ends() {
    let s = PrintScene::new(0);
    for t in PRINT_AT..CARRY_AT {
        assert!(!s.frame(t).paper_in_front, "at {t}");
    }
    assert!(s.frame(CARRY_AT).paper_in_front);
    assert!(s.frame(REST_AT).paper_in_front);
}

#[test]
fn cut_follows_the_fold() {
    let s = PrintScene::new(0);
    let mut moved = 0;
    for t in RIP_AT..CARRY_AT {
        let p = s.frame(t).paper.unwrap();
        let want = ((p.fold - 0.78 * CUT) / 0.62).max(-1.0);
        assert!(near(p.tear, want), "at {t}: {} vs {want}", p.tear);
        moved += (p.tear > 0.0) as u32;
    }
    assert!(moved > 100);
}

#[test]
fn no_paper_during_the_gap() {
    let s = PrintScene::new(0);
    assert!(s.frame(GAP_AT - 1).paper.is_some());
    for t in GAP_AT..RETURN_AT {
        assert!(s.frame(t).paper.is_none(), "at {t}");
    }
    assert!(s.frame(RETURN_AT).paper.is_some());
}

#[test]
fn rests_until_stop() {
    let s = PrintScene::new(0);
    let rest = s.frame(REST_AT);
    assert_eq!(s.frame(REST_AT + 3_600_000), rest);
    assert!(!s.done(REST_AT + 3_600_000));
}

#[test]
fn tear_cue_at_710_ms_into_rip() {
    let mut s = PrintScene::new(0);
    assert!(s.cues(RIP_AT + 709).iter().all(|c| *c != Cue::Tear));
    assert!(s.cues(RIP_AT + 710).contains(&Cue::Tear));
}

#[test]
fn buzz_cue_starts_with_the_print() {
    let mut s = PrintScene::new(0);
    assert!(s.cues(PRINT_AT - 1).is_empty());
    assert_eq!(s.cues(PRINT_AT), vec![Cue::Buzz]);
}

#[test]
fn each_cue_fires_once() {
    let mut s = PrintScene::new(0);
    let mut all = Vec::new();
    for t in 0..REST_AT + 1000 {
        all.extend(s.cues(t));
    }
    assert_eq!(all, vec![Cue::Buzz, Cue::Tear]);
    s.stop(REST_AT + 1000);
    let mut after = Vec::new();
    for t in REST_AT + 1000..REST_AT + 4000 {
        after.extend(s.cues(t));
    }
    assert_eq!(after, vec![Cue::BuzzOff]);
}

#[test]
fn a_stop_before_the_print_stays_silent() {
    let mut s = PrintScene::new(0);
    s.stop(500);
    for t in 500..6000 {
        assert!(s.cues(t).is_empty());
    }
}

#[test]
fn a_stop_during_print_silences_the_buzz_and_skips_the_tear() {
    let mut s = PrintScene::new(0);
    assert_eq!(s.cues(PRINT_AT + 100), vec![Cue::Buzz]);
    s.stop(PRINT_AT + 100);
    let mut after = Vec::new();
    for t in PRINT_AT + 100..RIP_AT + 3000 {
        after.extend(s.cues(t));
    }
    assert_eq!(after, vec![Cue::BuzzOff]);
}

fn smooth_from(s: &PrintScene, from: u64) {
    let mut last = s.frame(from);
    let mut t = from;
    while !s.done(t) {
        t += 1;
        let f = s.frame(t);
        assert!(
            (f.printer_y - last.printer_y).abs() <= 4.0,
            "printer jumps at {t}: {} to {}",
            last.printer_y,
            f.printer_y
        );
        last = f;
    }
}

#[test]
fn one_poll_never_starts_and_silences_the_buzz() {
    let mut s = PrintScene::new(0);
    s.stop(PRINT_AT + 100);
    assert!(s.cues(PRINT_AT + 200).is_empty());
    assert!(s.cues(RIP_AT + 3000).is_empty());
}

fn continuous_from(s: &PrintScene, stop: u64) {
    let mut t = stop.saturating_sub(1);
    let mut last = s.frame(t);
    while !s.done(t) {
        t += 1;
        let f = s.frame(t);
        assert!(
            (f.printer_y - last.printer_y).abs() <= 4.0,
            "stop {stop}: printer jumps at {t}: {} to {}",
            last.printer_y,
            f.printer_y
        );
        if let (Some(a), Some(b)) = (last.paper, f.paper) {
            assert!(
                (a.pivot.1 - b.pivot.1).abs() <= 8.0,
                "stop {stop}: paper jumps at {t}: {} to {}",
                a.pivot.1,
                b.pivot.1
            );
        }
        last = f;
    }
}

#[test]
fn a_stop_at_any_moment_stays_continuous() {
    for stop in (0..=REST_AT + 100).step_by(20) {
        let mut s = PrintScene::new(0);
        s.stop(stop);
        continuous_from(&s, stop);
    }
}

#[test]
fn stop_during_print_clears_paper_then_drops() {
    let stop = PRINT_AT + 2400;
    let mut s = PrintScene::new(0);
    s.stop(stop);
    let start_y = s.frame(stop).paper.unwrap().pivot.1;
    let mut last = start_y;
    for t in stop..stop + 380 {
        let f = s.frame(t);
        let p = f.paper.unwrap();
        assert!(!f.paper_in_front);
        assert!(near(f.printer_y, 0.0));
        assert!(p.pivot.1 >= last);
        last = p.pivot.1;
    }
    assert!(
        last - 425.0 > 292.0,
        "the sheet top is still at {}",
        last - 425.0
    );
    assert!(s.frame(stop + 380).paper.is_none());
    assert!(near(s.frame(stop + 380 + 60).printer_y, 0.0));
    assert!(s.frame(stop + 380 + 60 + 240).printer_y > 0.0);
    smooth_from(&s, stop);
}

#[test]
fn stop_during_hold_retracts_the_full_sheet() {
    let stop = RIP_AT - 10;
    let mut s = PrintScene::new(0);
    s.stop(stop);
    let end = s.frame(stop + 379).paper.unwrap().pivot.1;
    assert!(
        end - 425.0 > 292.0,
        "the sheet top is still at {}",
        end - 425.0
    );
}

#[test]
fn stop_during_rip_finishes_leaving_the_screen() {
    let stop = RIP_AT + 200;
    let mut s = PrintScene::new(0);
    let open = PrintScene::new(0);
    s.stop(stop);
    for t in stop..GAP_AT {
        assert_eq!(s.frame(t), open.frame(t), "at {t}");
    }
    assert!(s.frame(GAP_AT).paper.is_none());
    assert!(s.frame(GAP_AT + 380).paper.is_none());
    let mut t = stop;
    while !s.done(t) {
        t += 1;
        assert!(s.frame(t).paper.map_or(true, |p| !p.bow), "at {t}");
    }
    smooth_from(&s, stop);
    let mut cues = Vec::new();
    for t in stop..t {
        cues.extend(s.cues(t));
    }
    assert!(cues.contains(&Cue::Tear));
}

#[test]
fn stop_during_return_falls_away() {
    let stop = RETURN_AT + 280;
    let mut s = PrintScene::new(0);
    s.stop(stop);
    let start = s.frame(stop).paper.unwrap();
    assert!(start.bow);
    let mid = s.frame(stop + 300).paper.unwrap();
    assert!(mid.pivot.1 > start.pivot.1);
    assert!(s.frame(stop + 300).paper_in_front);
    assert!(s.frame(stop + 380).paper.is_none());
}

#[test]
fn stop_at_rest_runs_the_mockup_path() {
    let stop = REST_AT + 500;
    let mut s = PrintScene::new(0);
    s.stop(stop);
    let f = s.frame(stop + 190);
    assert!(f.lamp_on);
    assert!(f.paper_in_front);
    let p = f.paper.unwrap();
    assert_eq!(p.pivot.0, 668.0);
    assert!(near(p.pivot.1, 462.0 + (960.0 - 462.0) * 0.125));
    let dark = s.frame(stop + 380);
    assert!(dark.paper.is_none());
    assert!(!dark.lamp_on);
    assert!(near(dark.printer_y, 0.0));
    assert!(near(s.frame(stop + 440 + 240).printer_y, 490.0 * 0.125));
    assert!(near(s.frame(stop + 920).printer_y, 490.0));
    assert!(near(s.frame(stop + 920).slide, 0.0));
    assert!(near(s.frame(stop + 1050).slide, 0.5));
    smooth_from(&s, stop);
}

#[test]
fn stop_during_the_rise_drops_from_where_it_is() {
    let stop = 260 + 200;
    let mut s = PrintScene::new(0);
    let y = s.frame(stop).printer_y;
    s.stop(stop);
    assert!(near(s.frame(stop).printer_y, y));
    assert!(s.frame(stop + 480 + 120).slide > 0.0);
    smooth_from(&s, stop);
    assert!(s.done(stop + 480 + 260));
}

#[test]
fn stop_during_the_slide_returns_to_the_menu() {
    let mut s = PrintScene::new(0);
    s.stop(130);
    assert!(near(s.frame(130).slide, 0.5));
    assert!(near(s.frame(195).slide, 0.75));
    assert!(!s.done(259));
    assert!(s.done(260));
    assert!(near(s.frame(260).slide, 1.0));
}

#[test]
fn a_second_stop_changes_nothing() {
    let mut s = PrintScene::new(0);
    s.stop(REST_AT + 10);
    let before = s.frame(REST_AT + 500);
    s.stop(REST_AT + 400);
    assert_eq!(s.frame(REST_AT + 500), before);
}

#[test]
fn done_after_the_scene_slides_away() {
    let stop = REST_AT + 500;
    let mut s = PrintScene::new(0);
    assert!(!s.done(stop));
    s.stop(stop);
    let end = stop + 380 + 60 + 480 + 260;
    assert!(!s.done(end - 1));
    assert!(s.done(end));
    assert!(near(s.frame(end).slide, 1.0));
    assert!(near(s.frame(end).printer_y, 490.0));
    assert!(s.frame(end).paper.is_none());
}

#[test]
fn printing_waits_for_the_paper() {
    let mut s = PrintScene::new(0);
    assert!(s.cues(PRINT_AT - 1).is_empty());
    s.hold_print(PRINT_AT - 1);
    s.hold_print(PRINT_AT + 500);
    assert!(s.cues(PRINT_AT + 500).is_empty());
    assert!(s.frame(PRINT_AT + 500).lamp_on);
    assert_eq!(s.cues(PRINT_AT + 501), vec![Cue::Buzz]);
}

#[test]
fn a_stopped_scene_is_not_held() {
    let mut s = PrintScene::new(0);
    s.stop(PRINT_AT + 100);
    let f = s.frame(PRINT_AT + 300);
    s.hold_print(PRINT_AT + 300);
    assert_eq!(s.frame(PRINT_AT + 300), f);
}
