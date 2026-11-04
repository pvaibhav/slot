use std::ffi::CStr;
use std::net::Ipv4Addr;

const DOMAIN: &str = ".local";
const BASEOS_CONF: &str = "/run/baseos.conf";

pub fn wifi_ip() -> Option<Ipv4Addr> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return None;
    }
    let mut wlan = None;
    let mut other = None;
    let mut cur = head;
    while !cur.is_null() {
        let ifa = unsafe { &*cur };
        cur = ifa.ifa_next;
        let up = ifa.ifa_flags & libc::IFF_UP as u32 != 0;
        let loopback = ifa.ifa_flags & libc::IFF_LOOPBACK as u32 != 0;
        if !up || loopback || ifa.ifa_addr.is_null() || ifa.ifa_name.is_null() {
            continue;
        }
        if unsafe { (*ifa.ifa_addr).sa_family } as i32 != libc::AF_INET {
            continue;
        }
        let sin = unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in) };
        let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
        if ip.is_unspecified() || ip.is_loopback() {
            continue;
        }
        let name = unsafe { CStr::from_ptr(ifa.ifa_name) };
        if name.to_bytes() == b"wlan0" {
            wlan = Some(ip);
            break;
        }
        if other.is_none() {
            other = Some(ip);
        }
    }
    unsafe { libc::freeifaddrs(head) };
    if cfg!(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm"))) {
        wlan
    } else {
        wlan.or(other)
    }
}

pub fn model_name(model: &str) -> Option<String> {
    let first = model.split('/').next()?;
    let name: String = first.split_whitespace().collect::<String>().to_ascii_lowercase();
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric())).then_some(name)
}

pub fn mdns_host(hostname: &str, conf: Option<&str>) -> Option<String> {
    if conf.and_then(|c| crate::baseos::lookup(c, "mdns")).as_deref() == Some("false") {
        return None;
    }
    let name = hostname.trim();
    let name = name.strip_suffix(DOMAIN).unwrap_or(name);
    (!name.is_empty()).then(|| format!("{name}{DOMAIN}").to_uppercase())
}

fn system_hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } != 0 {
        return None;
    }
    Some(CStr::from_bytes_until_nul(&buf).ok()?.to_str().ok()?.to_string())
}

pub fn local_host() -> Option<String> {
    mdns_host(&system_hostname()?, std::fs::read_to_string(BASEOS_CONF).ok().as_deref())
}
