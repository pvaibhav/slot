use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slot::net_time::{NetTime, EVERY_MS};

fn counting(answer: Option<i64>) -> (NetTime, Arc<AtomicUsize>) {
    let asked = Arc::new(AtomicUsize::new(0));
    let seen = asked.clone();
    let source = Arc::new(move || {
        seen.fetch_add(1, Ordering::SeqCst);
        answer
    });
    (NetTime::new(source), asked)
}

fn answer(n: &mut NetTime, ready: bool, now: u64) -> Option<i64> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(t) = n.step(ready, now) {
            return Some(t);
        }
        if !n.waiting() {
            return None;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    None
}

#[test]
fn it_never_asks_until_it_is_ready() {
    let (mut n, asked) = counting(Some(5));
    assert_eq!(n.step(false, 0), None);
    assert_eq!(asked.load(Ordering::SeqCst), 0);
}

#[test]
fn being_ready_asks_and_gives_the_answer() {
    let (mut n, _) = counting(Some(5));
    assert_eq!(answer(&mut n, true, 0), Some(5));
}

#[test]
fn it_asks_once_a_day_while_ready() {
    let (mut n, asked) = counting(Some(5));
    answer(&mut n, true, 0);
    n.step(true, EVERY_MS - 1);
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert_eq!(answer(&mut n, true, EVERY_MS), Some(5));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[test]
fn losing_the_connection_asks_again_on_the_next_one() {
    let (mut n, asked) = counting(Some(5));
    answer(&mut n, true, 0);
    n.step(false, 10);
    assert_eq!(answer(&mut n, true, 20), Some(5));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[test]
fn a_failed_ask_gives_nothing_and_waits_for_the_next_turn() {
    let (mut n, asked) = counting(None);
    assert_eq!(answer(&mut n, true, 0), None);
    n.step(true, 10);
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}
