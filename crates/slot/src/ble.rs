use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use slot_input::Millis;

use crate::invite::{decode, nickname_of, Advert};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heard {
    pub advert: Advert,
    pub nickname: Option<String>,
    pub rssi: i8,
    pub at: Millis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BleWant {
    Off,
    Ready,
    Listen,
    Advertise { advert: [u8; 31], scan: Vec<u8> },
}

pub trait Ble: Send {
    fn set(&mut self, want: BleWant);
    fn heard(&mut self) -> Vec<Heard>;
}

pub fn clock() -> Millis {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as Millis
}

pub fn cmd(ogf: u8, ocf: u16, params: &[u8]) -> Vec<u8> {
    let opcode = ((ogf as u16) << 10) | (ocf & 0x03FF);
    let mut out = Vec::with_capacity(params.len() + 4);
    out.push(0x01);
    out.extend_from_slice(&opcode.to_le_bytes());
    out.push(params.len() as u8);
    out.extend_from_slice(params);
    out
}

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct Report {
    kind: u8,
    addr: [u8; 6],
    data: Vec<u8>,
    rssi: i8,
}

fn parse(event: &[u8]) -> Vec<Report> {
    let e = if event.first() == Some(&0x04) && event.get(1) == Some(&0x3E) {
        &event[1..]
    } else {
        event
    };
    if e.len() < 4 || e[0] != 0x3E || e[2] != 0x02 {
        return Vec::new();
    }
    let e = &e[..e.len().min(e[1] as usize + 2)];
    let mut out = Vec::new();
    let mut i = 4;
    for _ in 0..e[3] {
        if i + 9 > e.len() {
            break;
        }
        let kind = e[i];
        let mut addr = [0u8; 6];
        addr.copy_from_slice(&e[i + 2..i + 8]);
        let len = e[i + 8] as usize;
        i += 9;
        if i + len + 1 > e.len() {
            break;
        }
        out.push(Report {
            kind,
            addr,
            data: e[i..i + len].to_vec(),
            rssi: e[i + len] as i8,
        });
        i += len + 1;
    }
    out
}

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
const ADV_IND: u8 = 0x00;
#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
const SCAN_RSP: u8 = 0x04;

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
struct Seen {
    advert: Advert,
    first: f64,
}

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
#[derive(Default)]
struct Joiner {
    seen: HashMap<[u8; 6], Seen>,
    names: HashMap<[u8; 6], String>,
}

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
fn addr_text(addr: &[u8; 6]) -> String {
    addr.iter()
        .rev()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
impl Joiner {
    fn clear(&mut self) {
        self.seen.clear();
        self.names.clear();
    }

    fn take(
        &mut self,
        kind: u8,
        addr: [u8; 6],
        data: &[u8],
        rssi: i8,
        now: Millis,
    ) -> Option<Heard> {
        match kind {
            ADV_IND => {
                let advert = decode(data)?;
                let nickname = self.names.get(&addr).cloned();
                let first = match self.seen.get(&addr) {
                    Some(s) => s.first,
                    None => {
                        let at = crate::boot_time::ms();
                        eprintln!(
                            "slot: invite: ble advert from {} at {at:.0} ms, handed out at once, {}",
                            addr_text(&addr),
                            match &nickname {
                                Some(name) => format!("nickname {name:?}"),
                                None => "no nickname yet".to_string(),
                            }
                        );
                        at
                    }
                };
                self.seen.insert(addr, Seen { advert, first });
                Some(Heard {
                    advert,
                    nickname,
                    rssi,
                    at: now,
                })
            }
            SCAN_RSP => {
                let name = nickname_of(data)?;
                let seen = self.seen.get(&addr)?;
                let (advert, first) = (seen.advert, seen.first);
                if self.names.get(&addr) != Some(&name) {
                    let at = crate::boot_time::ms();
                    eprintln!(
                        "slot: invite: ble nickname {name:?} from {} at {at:.0} ms, {:.0} ms after its first advert",
                        addr_text(&addr),
                        at - first
                    );
                    self.names.insert(addr, name.clone());
                }
                Some(Heard {
                    advert,
                    nickname: Some(name),
                    rssi,
                    at: now,
                })
            }
            _ => None,
        }
    }
}

pub fn reports(event: &[u8]) -> Vec<(Vec<u8>, i8)> {
    parse(event).into_iter().map(|r| (r.data, r.rssi)).collect()
}

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
const RETRY_MS: Millis = 10_000;

#[cfg_attr(not(all(feature = "device", not(feature = "nds"))), allow(dead_code))]
fn retry_in(asked: &BleWant, failed_at: Option<Millis>, now: Millis) -> Option<Millis> {
    if *asked == BleWant::Off {
        return None;
    }
    let at = failed_at?.saturating_add(RETRY_MS);
    Some(at.saturating_sub(now))
}

pub struct NoBle;

impl Ble for NoBle {
    fn set(&mut self, _want: BleWant) {}

    fn heard(&mut self) -> Vec<Heard> {
        Vec::new()
    }
}

#[derive(Default)]
struct FakeState {
    wants: Vec<BleWant>,
    heard: Vec<Heard>,
}

pub struct FakeBle {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Clone)]
pub struct FakeBleRemote {
    state: Arc<Mutex<FakeState>>,
}

impl FakeBle {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (FakeBle, FakeBleRemote) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        (
            FakeBle {
                state: state.clone(),
            },
            FakeBleRemote { state },
        )
    }
}

impl Ble for FakeBle {
    fn set(&mut self, want: BleWant) {
        if let Ok(mut s) = self.state.lock() {
            s.wants.push(want);
        }
    }

    fn heard(&mut self) -> Vec<Heard> {
        self.state
            .lock()
            .map(|mut s| std::mem::take(&mut s.heard))
            .unwrap_or_default()
    }
}

impl FakeBleRemote {
    pub fn hear(&self, heard: Heard) {
        if let Ok(mut s) = self.state.lock() {
            s.heard.push(heard);
        }
    }

    pub fn wants(&self) -> Vec<BleWant> {
        self.state
            .lock()
            .map(|s| s.wants.clone())
            .unwrap_or_default()
    }
}

#[cfg(not(all(feature = "device", not(feature = "nds"))))]
pub fn ble() -> Box<dyn Ble> {
    Box::new(NoBle)
}

#[cfg(all(feature = "device", not(feature = "nds")))]
pub use device::{ble, probe};

#[cfg(all(feature = "device", not(feature = "nds")))]
mod device {
    use std::collections::VecDeque;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::path::Path;
    use std::process::Command;
    use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::Duration;

    use slot_input::Millis;
    use slot_store::{Core, Platform};

    use super::{clock, cmd, parse, retry_in, Ble, BleWant, Heard, Joiner};
    use crate::invite::{encode, scan_response, Advert};
    use crate::link_kind::LinkKind;

    const AF_BLUETOOTH: libc::c_int = 31;
    const BTPROTO_HCI: libc::c_int = 1;
    const HCI_CHANNEL_RAW: u16 = 0;
    const SOL_HCI: libc::c_int = 0;
    const HCI_FILTER: libc::c_int = 2;
    const HCI_EVENT_PKT: u8 = 0x04;
    const EVT_CMD_COMPLETE: u8 = 0x0E;
    const EVT_CMD_STATUS: u8 = 0x0F;
    const EVT_LE_META: u8 = 0x3E;
    const OGF_LE: u8 = 0x08;
    const LE_SET_ADV_PARAMS: u16 = 0x0006;
    const LE_SET_ADV_DATA: u16 = 0x0008;
    const LE_SET_SCAN_RSP_DATA: u16 = 0x0009;
    const LE_SET_ADV_ENABLE: u16 = 0x000A;
    const LE_SET_SCAN_PARAMS: u16 = 0x000B;
    const LE_SET_SCAN_ENABLE: u16 = 0x000C;
    const HEARD_MAX: usize = 64;
    const CMD_WAIT_MS: Millis = 1000;

    #[repr(C)]
    struct SockaddrHci {
        hci_family: libc::sa_family_t,
        hci_dev: u16,
        hci_channel: u16,
    }

    #[repr(C)]
    struct HciFilter {
        type_mask: u32,
        event_mask: [u32; 2],
        opcode: u16,
    }

    struct Hci {
        fd: OwnedFd,
    }

    fn os(what: &str) -> String {
        format!("{what}: {}", io::Error::last_os_error())
    }

    impl Hci {
        fn open() -> Result<Hci, String> {
            let raw = unsafe { libc::socket(AF_BLUETOOTH, libc::SOCK_RAW, BTPROTO_HCI) };
            if raw < 0 {
                return Err(os("socket"));
            }
            let fd = unsafe { OwnedFd::from_raw_fd(raw) };
            unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) };
            let addr = SockaddrHci {
                hci_family: AF_BLUETOOTH as libc::sa_family_t,
                hci_dev: 0,
                hci_channel: HCI_CHANNEL_RAW,
            };
            let bound = unsafe {
                libc::bind(
                    raw,
                    &addr as *const SockaddrHci as *const libc::sockaddr,
                    std::mem::size_of::<SockaddrHci>() as libc::socklen_t,
                )
            };
            if bound < 0 {
                return Err(os("bind hci0"));
            }
            let mut event_mask = [0u32; 2];
            for code in [EVT_CMD_COMPLETE, EVT_CMD_STATUS, EVT_LE_META] {
                event_mask[(code / 32) as usize] |= 1 << (code % 32);
            }
            let filter = HciFilter {
                type_mask: 1 << HCI_EVENT_PKT,
                event_mask,
                opcode: 0,
            };
            let set = unsafe {
                libc::setsockopt(
                    raw,
                    SOL_HCI,
                    HCI_FILTER,
                    &filter as *const HciFilter as *const libc::c_void,
                    std::mem::size_of::<HciFilter>() as libc::socklen_t,
                )
            };
            if set < 0 {
                return Err(os("hci filter"));
            }
            Ok(Hci { fd })
        }

        fn read(&self, timeout_ms: i32) -> Result<Option<Vec<u8>>, String> {
            let mut pfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let n = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    return Ok(None);
                }
                return Err(format!("poll: {e}"));
            }
            if n == 0 {
                return Ok(None);
            }
            if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0
                && pfd.revents & libc::POLLIN == 0
            {
                return Err(format!("hci socket closed ({:#x})", pfd.revents));
            }
            let mut buf = vec![0u8; 260];
            let got = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if got < 0 {
                return Err(os("read"));
            }
            buf.truncate(got as usize);
            Ok(Some(buf))
        }

        fn send(&self, ocf: u16, params: &[u8]) -> Result<(), String> {
            let packet = cmd(OGF_LE, ocf, params);
            let wrote = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    packet.as_ptr() as *const libc::c_void,
                    packet.len(),
                )
            };
            if wrote != packet.len() as isize {
                return Err(os(&format!("write {ocf:#06x}")));
            }
            let opcode = [packet[1], packet[2]];
            let end = clock() + CMD_WAIT_MS;
            while clock() < end {
                let Some(e) = self.read(100)? else {
                    continue;
                };
                let status = match e.get(1) {
                    Some(&EVT_CMD_COMPLETE) if e.len() >= 7 && e[4..6] == opcode => e[6],
                    Some(&EVT_CMD_STATUS) if e.len() >= 7 && e[5..7] == opcode => e[3],
                    _ => continue,
                };
                if status != 0 {
                    return Err(format!("command {ocf:#06x} status {status:#04x}"));
                }
                return Ok(());
            }
            Err(format!("command {ocf:#06x} not answered"))
        }

        fn quiet(&self) {
            let _ = self.send(LE_SET_ADV_ENABLE, &[0]);
            let _ = self.send(LE_SET_SCAN_ENABLE, &[0, 0]);
        }

        fn listen(&self) -> Result<(), String> {
            self.send(
                LE_SET_SCAN_PARAMS,
                &[0x01, 0x60, 0x00, 0x30, 0x00, 0x00, 0x00],
            )?;
            self.send(LE_SET_SCAN_ENABLE, &[0x01, 0x00])
        }

        fn advertise(&self, advert: &[u8; 31], scan: &[u8]) -> Result<(), String> {
            let mut params = vec![0xA0, 0x00, 0xF0, 0x00, 0x00, 0x00, 0x00];
            params.extend_from_slice(&[0; 6]);
            params.extend_from_slice(&[0x07, 0x00]);
            self.send(LE_SET_ADV_PARAMS, &params)?;
            let mut data = [0u8; 32];
            data[0] = 31;
            data[1..].copy_from_slice(advert);
            self.send(LE_SET_ADV_DATA, &data)?;
            let scan = &scan[..scan.len().min(31)];
            let mut rsp = [0u8; 32];
            rsp[0] = scan.len() as u8;
            rsp[1..=scan.len()].copy_from_slice(scan);
            self.send(LE_SET_SCAN_RSP_DATA, &rsp)?;
            self.send(LE_SET_ADV_ENABLE, &[0x01])
        }
    }

    fn bt(verb: &str) -> bool {
        let root = std::env::var_os("SLOT_ROOT").unwrap_or_else(|| "/mnt/sdcard".into());
        Command::new("/bin/sh")
            .arg(Path::new(&root).join("System/slotlink.sh"))
            .arg("bt")
            .arg(verb)
            .status()
            .is_ok_and(|s| s.success())
    }

    type Queue = Arc<Mutex<VecDeque<Heard>>>;

    struct Worker {
        hci: Option<Hci>,
        powered: bool,
        asked: BleWant,
        failed_at: Option<Millis>,
        heard: Queue,
        joiner: Joiner,
    }

    impl Worker {
        fn push(&self, heard: Heard) {
            if let Ok(mut q) = self.heard.lock() {
                q.push_back(heard);
                while q.len() > HEARD_MAX {
                    q.pop_front();
                }
            }
        }

        fn apply(&mut self, want: BleWant) {
            self.asked = want.clone();
            self.failed_at = None;
            self.joiner.clear();
            if want == BleWant::Off {
                self.stop();
                return;
            }
            let cold = self.hci.is_none();
            let from = crate::boot_time::ms();
            if let Err(e) = self.start(&want) {
                eprintln!("slot: ble: {e}, Bluetooth left off, retrying");
                self.stop();
                self.failed_at = Some(clock());
                return;
            }
            let at = crate::boot_time::ms();
            let what = match want {
                BleWant::Ready => "ready",
                BleWant::Listen => "listening",
                BleWant::Advertise { .. } => "advertising",
                BleWant::Off => "off",
            };
            eprintln!(
                "slot: invite: ble {what} at {at:.0} ms, {:.0} ms to start{}",
                at - from,
                if cold { " with bt up" } else { "" }
            );
        }

        fn start(&mut self, want: &BleWant) -> Result<(), String> {
            if self.hci.is_none() {
                self.powered = true;
                if !bt("up") {
                    return Err("bt up failed".into());
                }
                self.hci = Some(Hci::open()?);
            }
            let Some(hci) = &self.hci else {
                return Err("no hci".into());
            };
            hci.quiet();
            match want {
                BleWant::Listen => hci.listen(),
                BleWant::Advertise { advert, scan } => hci.advertise(advert, scan),
                BleWant::Ready | BleWant::Off => Ok(()),
            }
        }

        fn stop(&mut self) {
            if let Some(hci) = self.hci.take() {
                hci.quiet();
            }
            if self.powered {
                self.powered = false;
                bt("down");
            }
        }

        fn listening(&self) -> bool {
            self.hci.is_some() && self.asked == BleWant::Listen
        }

        fn listen_once(&mut self) {
            let got = match &self.hci {
                Some(hci) => hci.read(100),
                None => return,
            };
            match got {
                Ok(Some(event)) => {
                    let now = clock();
                    for r in parse(&event) {
                        self.take(r.kind, r.addr, &r.data, r.rssi, now);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("slot: ble: {e}, Bluetooth left off, retrying");
                    self.stop();
                    self.failed_at = Some(clock());
                }
            }
        }

        fn take(&mut self, kind: u8, addr: [u8; 6], data: &[u8], rssi: i8, now: Millis) {
            if let Some(heard) = self.joiner.take(kind, addr, data, rssi, now) {
                self.push(heard);
            }
        }
    }

    fn work(rx: Receiver<BleWant>, heard: Queue) {
        let mut w = Worker {
            hci: None,
            powered: false,
            asked: BleWant::Off,
            failed_at: None,
            heard,
            joiner: Joiner::default(),
        };
        loop {
            let next = if w.listening() {
                w.listen_once();
                match rx.try_recv() {
                    Ok(want) => Some(want),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                }
            } else if let Some(wait) = retry_in(&w.asked, w.failed_at, clock()) {
                match rx.recv_timeout(Duration::from_millis(wait)) {
                    Ok(want) => Some(want),
                    Err(RecvTimeoutError::Timeout) => {
                        w.apply(w.asked.clone());
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(want) => Some(want),
                    Err(_) => break,
                }
            };
            let Some(mut want) = next else {
                continue;
            };
            while let Ok(later) = rx.try_recv() {
                want = later;
            }
            if want != w.asked {
                w.apply(want);
            }
        }
        w.apply(BleWant::Off);
    }

    struct HciBle {
        tx: Option<Sender<BleWant>>,
        last: BleWant,
        heard: Queue,
        worker: Option<JoinHandle<()>>,
    }

    impl Ble for HciBle {
        fn set(&mut self, want: BleWant) {
            if want == self.last {
                return;
            }
            self.last = want.clone();
            if let Some(tx) = &self.tx {
                let _ = tx.send(want);
            }
        }

        fn heard(&mut self) -> Vec<Heard> {
            self.heard
                .lock()
                .map(|mut q| q.drain(..).collect())
                .unwrap_or_default()
        }
    }

    impl Drop for HciBle {
        fn drop(&mut self) {
            self.tx = None;
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    pub fn ble() -> Box<dyn Ble> {
        let (tx, rx) = channel();
        let heard: Queue = Arc::new(Mutex::new(VecDeque::new()));
        let theirs = heard.clone();
        let worker = std::thread::Builder::new()
            .name("slot-ble".into())
            .spawn(move || work(rx, theirs))
            .ok();
        if worker.is_none() {
            eprintln!("slot: ble: worker would not start");
        }
        Box::new(HciBle {
            tx: worker.as_ref().map(|_| tx),
            last: BleWant::Off,
            heard,
            worker,
        })
    }

    pub fn probe(mode: &str) {
        let advert = Advert {
            session: 0x1234,
            game_crc: 0xDEAD_BEEF,
            core: Core::Gpsp,
            bios_crc: 0x81977335,
            kind: LinkKind::Wireless,
            freq_mhz: 2437,
            bssid: [0xA0, 0xB1, 0xC2, 0xD3, 0xE4, 0xF5],
            platform: Platform::Gba,
        };
        let want = match mode {
            "advertise" => BleWant::Advertise {
                advert: encode(&advert),
                scan: scan_response("slot probe"),
            },
            "listen" => BleWant::Listen,
            _ => {
                eprintln!("slot: ble: SLOT_BLE_PROBE is advertise or listen, not {mode:?}");
                return;
            }
        };
        eprintln!("slot: ble: probe {mode} for 20 s");
        let mut b = ble();
        b.set(want);
        let end = clock() + 20_000;
        let mut count = 0usize;
        while clock() < end {
            for h in b.heard() {
                count += 1;
                eprintln!(
                    "slot: ble: heard {:?} nickname {:?} rssi {} at {}",
                    h.advert, h.nickname, h.rssi, h.at
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        b.set(BleWant::Off);
        drop(b);
        eprintln!("slot: ble: probe done, heard {count}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invite::{encode, scan_response};
    use crate::link_kind::LinkKind;
    use slot_store::{Core, Platform};

    fn sample() -> Advert {
        Advert {
            session: 0x1234,
            game_crc: 0xDEAD_BEEF,
            core: Core::Gpsp,
            bios_crc: 0x81977335,
            kind: LinkKind::Wireless,
            freq_mhz: 2437,
            bssid: [0xA0, 0xB1, 0xC2, 0xD3, 0xE4, 0xF5],
            platform: Platform::Gba,
        }
    }

    fn report(kind: u8, addr: u8, data: &[u8], rssi: u8) -> Vec<u8> {
        let mut r = vec![kind, 0x00, addr, 2, 3, 4, 5, 6, data.len() as u8];
        r.extend_from_slice(data);
        r.push(rssi);
        r
    }

    fn event(reports: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = reports.concat();
        let mut e = vec![
            0x04,
            0x3E,
            (body.len() + 2) as u8,
            0x02,
            reports.len() as u8,
        ];
        e.extend_from_slice(&body);
        e
    }

    #[test]
    fn a_command_packet_has_type_opcode_and_length() {
        assert_eq!(
            cmd(0x08, 0x000C, &[1, 0]),
            vec![0x01, 0x0C, 0x20, 0x02, 0x01, 0x00]
        );
    }

    #[test]
    fn an_advertising_report_yields_data_and_rssi() {
        let data = encode(&sample());
        let e = event(&[report(0x00, 1, &data, 0xC4)]);
        assert_eq!(reports(&e), vec![(data.to_vec(), -60)]);
        assert_eq!(reports(&e[1..]), vec![(data.to_vec(), -60)]);
    }

    #[test]
    fn two_reports_in_one_event_both_come_out() {
        let data = encode(&sample());
        let name = scan_response("Kid");
        let e = event(&[report(0x00, 1, &data, 0xC4), report(0x04, 1, &name, 0xBA)]);
        assert_eq!(reports(&e), vec![(data.to_vec(), -60), (name.clone(), -70)]);
        let cut = &e[..e.len() - 3];
        assert_eq!(reports(cut), vec![(data.to_vec(), -60)]);
        assert!(reports(&[0x04, 0x0E, 0x04, 0x01, 0x0C, 0x20, 0x00]).is_empty());
    }

    #[test]
    fn fake_ble_records_wants_and_replays_heard() {
        let (mut fake, remote) = FakeBle::new();
        fake.set(BleWant::Listen);
        fake.set(BleWant::Off);
        assert_eq!(remote.wants(), vec![BleWant::Listen, BleWant::Off]);
        let h = Heard {
            advert: sample(),
            nickname: Some("Dad".into()),
            rssi: -50,
            at: 42,
        };
        remote.hear(h.clone());
        assert_eq!(fake.heard(), vec![h]);
        assert!(fake.heard().is_empty());
    }

    #[test]
    fn a_failed_want_retries_after_the_backoff() {
        assert_eq!(retry_in(&BleWant::Listen, None, 500), None);
        assert_eq!(retry_in(&BleWant::Listen, Some(1000), 1000), Some(RETRY_MS));
        assert_eq!(
            retry_in(&BleWant::Listen, Some(1000), 4000),
            Some(RETRY_MS - 3000)
        );
        assert_eq!(
            retry_in(&BleWant::Listen, Some(1000), 1000 + RETRY_MS),
            Some(0)
        );
        assert_eq!(retry_in(&BleWant::Listen, Some(1000), 90_000), Some(0));
        let advertise = BleWant::Advertise {
            advert: [0; 31],
            scan: Vec::new(),
        };
        assert_eq!(
            retry_in(&advertise, Some(1000), 2000),
            Some(RETRY_MS - 1000)
        );
        assert_eq!(retry_in(&BleWant::Off, Some(1000), 90_000), None);
    }

    #[test]
    fn an_advert_is_handed_out_before_its_scan_response() {
        let mut j = Joiner::default();
        let advert = encode(&sample());
        let heard = j
            .take(ADV_IND, [1; 6], &advert, -50, 10)
            .expect("the advert was held");
        assert_eq!(heard.advert, sample());
        assert_eq!(heard.nickname, None);
        assert_eq!(heard.at, 10);
    }

    #[test]
    fn a_scan_response_hands_out_the_advert_with_its_name() {
        let mut j = Joiner::default();
        j.take(ADV_IND, [1; 6], &encode(&sample()), -50, 10);
        let named = j
            .take(SCAN_RSP, [1; 6], &scan_response("kwallbmoc"), -52, 1500)
            .expect("a late scan response was dropped");
        assert_eq!(named.advert, sample());
        assert_eq!(named.nickname.as_deref(), Some("kwallbmoc"));
        assert_eq!(named.at, 1500);
        let later = j
            .take(ADV_IND, [1; 6], &encode(&sample()), -50, 1600)
            .unwrap();
        assert_eq!(later.nickname.as_deref(), Some("kwallbmoc"));
    }

    #[test]
    fn a_scan_response_from_a_stranger_is_ignored() {
        let mut j = Joiner::default();
        j.take(ADV_IND, [1; 6], &encode(&sample()), -50, 10);
        assert_eq!(
            j.take(SCAN_RSP, [2; 6], &scan_response("Phone"), -50, 20),
            None
        );
        assert_eq!(j.take(ADV_IND, [2; 6], &[2, 1, 6], -50, 30), None);
        j.clear();
        assert_eq!(
            j.take(SCAN_RSP, [1; 6], &scan_response("Dad"), -50, 40),
            None
        );
    }

    #[test]
    fn a_host_build_hears_nothing() {
        let mut b = NoBle;
        b.set(BleWant::Listen);
        assert!(b.heard().is_empty());
    }
}
