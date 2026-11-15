use std::time::{Duration, Instant};

const MARGIN: Duration = Duration::from_micros(1500);
const VBLANK_MARGIN: Duration = Duration::from_millis(5);
const WORK_WINDOW: usize = 32;

pub struct Pacer {
    last_return: Option<Instant>,
    period: Duration,
    work: Duration,
    works: [Duration; WORK_WINDOW],
    next: usize,
    learns: bool,
    vblank: Option<Instant>,
    aimed: Option<Instant>,
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
            vblank: None,
            aimed: None,
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

    pub fn lock_to(&mut self, vblank: Instant) {
        self.vblank = Some(vblank);
    }

    pub fn wait(&mut self) {
        let now = Instant::now();
        if let Some(left) = self.plan(now).and_then(|at| at.checked_duration_since(now)) {
            std::thread::sleep(left);
        }
    }

    pub fn plan(&mut self, now: Instant) -> Option<Instant> {
        let Some(vblank) = self.vblank else {
            let delay = self.period.saturating_sub(self.work + MARGIN);
            return self.last_return.map(|last| last + delay);
        };
        let mut earliest = now + self.work;
        if let Some(aimed) = self.aimed {
            earliest = earliest.max(aimed + self.period / 2);
        }
        let target = self.vblank_after(vblank, earliest);
        self.aimed = Some(target);
        Some((target - self.work - VBLANK_MARGIN).max(now))
    }

    fn vblank_after(&self, vblank: Instant, t: Instant) -> Instant {
        let period = self.period.as_nanos().max(1);
        match t.checked_duration_since(vblank) {
            Some(since) => {
                let n = since.as_nanos() / period + 1;
                vblank + Duration::from_nanos((n * period) as u64)
            }
            None => {
                let before = (vblank - t).as_nanos() / period;
                vblank - Duration::from_nanos((before * period) as u64)
            }
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

    fn record(
        &mut self,
        work: Duration,
        blocked: bool,
        seen: Option<Duration>,
        now: Instant,
    ) -> bool {
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
