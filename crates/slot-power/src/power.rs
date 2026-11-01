use std::path::PathBuf;
use std::time::Duration;

use crate::{Battery, Charge, LedState, LidPolicy, Platform};

pub struct Power {
    platform: Box<dyn Platform>,
    level: u8,
    closed: bool,
    timeout: Duration,
}

impl Power {
    pub fn new(platform: Box<dyn Platform>, timeout: Duration) -> Self {
        Power {
            platform,
            level: 0,
            closed: false,
            timeout,
        }
    }

    pub fn set_backlight(&mut self, step: u8) {
        self.level = step;
        if !self.closed {
            self.platform.set_backlight(step);
        }
    }

    pub fn battery(&self) -> Option<Battery> {
        self.platform.battery()
    }

    pub fn charge(&self) -> Charge {
        self.platform.charge()
    }

    pub fn headphones(&self) -> bool {
        self.platform.headphones()
    }

    pub fn card_read_only(&self) -> bool {
        self.platform.card_read_only()
    }

    pub fn card_log(&self) -> Option<PathBuf> {
        self.platform.card_log()
    }

    pub fn card_kernel_lines(&self) -> Vec<String> {
        self.platform.card_kernel_lines()
    }

    pub fn set_led(&mut self, state: LedState) {
        self.platform.set_led(state)
    }

    pub fn set_rumble(&mut self, strength: u16) {
        self.platform.set_rumble(strength);
    }

    pub fn relink_adb(&mut self) -> bool {
        self.platform.relink_adb()
    }

    pub fn poweroff(&mut self) -> ! {
        self.platform.poweroff()
    }

    pub fn restart(&mut self) -> ! {
        self.platform.restart()
    }

    pub fn repair_card(&mut self) -> ! {
        self.platform.repair_card()
    }

    pub fn take_repair_result(&mut self) -> Option<crate::RepairResult> {
        self.platform.take_repair_result()
    }

    pub fn now(&self) -> i64 {
        self.platform.now()
    }

    pub fn set_clock(&mut self, secs: i64) {
        self.platform.set_clock(secs);
    }

    pub fn set_clock_in_background(&mut self, secs: i64) {
        self.platform.set_clock_in_background(secs);
    }
}

impl LidPolicy for Power {
    fn on_close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.platform.set_backlight(0);
    }

    fn on_open(&mut self) {
        self.closed = false;
        self.platform.set_backlight(self.level);
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }
}
