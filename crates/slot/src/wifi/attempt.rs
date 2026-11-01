use std::time::{Duration, Instant};

use super::{State, Status};

pub const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    WrongPassword,
    NotFound,
    NoIp,
    Other,
}

impl Failure {
    pub fn label(&self) -> &'static str {
        match self {
            Failure::WrongPassword => "Wrong Password",
            Failure::NotFound => "Not Found",
            Failure::NoIp => "No IP Address",
            Failure::Other => "Could Not Connect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Connecting,
    Authenticating,
    GettingIp,
    Connected,
    Failed(Failure),
}

impl Phase {
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Connecting => "Connecting",
            Phase::Authenticating => "Authenticating",
            Phase::GettingIp => "Getting IP",
            Phase::Connected => "Connected",
            Phase::Failed(f) => f.label(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Attempt {
    pub id: u32,
    pub ssid: String,
    started: Instant,
    phase: Phase,
    associated: bool,
    handshook: bool,
}

impl Attempt {
    pub fn new(id: u32, ssid: &str, started: Instant) -> Attempt {
        Attempt {
            id,
            ssid: ssid.to_string(),
            started,
            phase: Phase::Connecting,
            associated: false,
            handshook: false,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn observe(
        &mut self,
        status: &Status,
        temp_disabled: bool,
        in_last_scan: bool,
        now: Instant,
    ) -> Phase {
        if self.done() {
            return self.phase;
        }
        let ours = status.ssid.as_deref() == Some(self.ssid.as_str());
        self.phase = match status.state {
            State::Completed if ours && status.ip.is_some() => Phase::Connected,
            State::Completed if ours => Phase::GettingIp,
            State::Handshake => Phase::Authenticating,
            _ => Phase::Connecting,
        };
        self.associated |= matches!(status.state, State::Associating | State::Handshake)
            || (ours && status.state == State::Completed);
        self.handshook |= status.state == State::Handshake;
        if self.phase == Phase::Connected {
            return self.phase;
        }
        if temp_disabled && self.handshook {
            self.phase = Phase::Failed(Failure::WrongPassword);
        } else if now.duration_since(self.started) >= TIMEOUT {
            self.phase = Phase::Failed(match self.phase {
                Phase::GettingIp => Failure::NoIp,
                _ if !self.associated && !in_last_scan => Failure::NotFound,
                _ => Failure::Other,
            });
        }
        self.phase
    }

    pub fn done(&self) -> bool {
        matches!(self.phase, Phase::Connected | Phase::Failed(_))
    }
}
