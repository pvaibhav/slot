use std::time::{Duration, Instant};

const MARGIN: Duration = Duration::from_micros(1500);
const WORK_WINDOW: usize = 32;

pub struct Pacer {
    last_return: Option<Instant>,
    period: Duration,
    work: Duration,
    works: [Duration; WORK_WINDOW],
    next: usize,
    learns: bool,
}

impl Default for Pacer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pacer {
    pub fn new() -> Self {
        Pacer {
            last_return: None,
            period: Duration::from_micros(16_760),
            work: Duration::from_millis(4),
            works: [Duration::from_millis(4); WORK_WINDOW],
            next: 0,
            learns: true,
        }
    }

    pub fn fixed(period: Duration) -> Self {
        Pacer {
            period,
            learns: false,
            ..Self::new()
        }
    }

    pub fn period(&self) -> Duration {
        self.period
    }

    pub fn work(&self) -> Duration {
        self.work
    }

    pub fn wait(&self) {
        let Some(last) = self.last_return else {
            return;
        };
        let delay = self.period.saturating_sub(self.work + MARGIN);
        if let Some(left) = (last + delay).checked_duration_since(Instant::now()) {
            std::thread::sleep(left);
        }
    }

    pub fn swapped(&mut self, work: Duration, blocked: bool) -> bool {
        let now = Instant::now();
        let seen = self.last_return.map(|last| now - last);
        self.record(work, blocked, seen, now)
    }

    pub fn swapped_after(&mut self, work: Duration, blocked: bool, seen: Duration) -> bool {
        self.record(work, blocked, Some(seen), Instant::now())
    }

    fn record(&mut self, work: Duration, blocked: bool, seen: Option<Duration>, now: Instant) -> bool {
        let mut missed = false;
        if let Some(seen) = seen {
            missed = seen > self.period.mul_f64(1.5);
            if self.learns
                && blocked
                && seen > Duration::from_millis(12)
                && seen < Duration::from_millis(22)
            {
                self.period = self.period.mul_f64(0.95) + seen.mul_f64(0.05);
            }
        }
        self.works[self.next] = work;
        self.next = (self.next + 1) % WORK_WINDOW;
        self.work = self.works.iter().copied().max().unwrap_or(work);
        self.last_return = Some(now);
        missed
    }
}
