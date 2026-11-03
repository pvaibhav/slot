const MGBA: usize = 48;
const GAMBATTE: usize = 8;

const WRAP: u64 = 0x1FF * 86_400;

pub fn convert(tail: &[u8], want: usize, now: u64) -> Option<Vec<u8>> {
    match (tail.len(), want) {
        (MGBA, GAMBATTE) => Some(mgba_to_gambatte(tail).to_le_bytes().to_vec()),
        (GAMBATTE, MGBA) => Some(gambatte_to_mgba(
            u64::from_le_bytes(tail.try_into().ok()?),
            now,
        )),
        _ => None,
    }
}

fn mgba_to_gambatte(tail: &[u8]) -> u64 {
    let reg = |i: usize| u64::from(tail[20 + i * 4]);
    let days = reg(3) + ((reg(4) & 1) << 8);
    let latched = reg(0) + 60 * reg(1) + 3_600 * reg(2) + 86_400 * days;
    let unix = u64::from_le_bytes(tail[40..48].try_into().unwrap());
    unix.wrapping_sub(latched)
}

fn gambatte_to_mgba(base: u64, now: u64) -> Vec<u8> {
    let mut t = now.wrapping_sub(base);
    if t > WRAP {
        t = (t - 1) % WRAP + 1;
    }
    let days = t / 86_400;
    let regs = [
        (t % 60) as u32,
        (t / 60 % 60) as u32,
        (t / 3_600 % 24) as u32,
        (days & 0xFF) as u32,
        (days >> 8) as u32,
    ];
    let mut out = Vec::with_capacity(MGBA);
    for r in regs.iter().chain(regs.iter()) {
        out.extend_from_slice(&r.to_le_bytes());
    }
    out.extend_from_slice(&now.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgba_tail(current: [u32; 5], latched: [u32; 5], unix: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(MGBA);
        for r in current.iter().chain(latched.iter()) {
            out.extend_from_slice(&r.to_le_bytes());
        }
        out.extend_from_slice(&unix.to_le_bytes());
        out
    }

    fn registers(tail: &[u8], at: usize) -> [u32; 5] {
        std::array::from_fn(|i| {
            u32::from_le_bytes(tail[at + i * 4..at + i * 4 + 4].try_into().unwrap())
        })
    }

    fn unix(tail: &[u8]) -> u64 {
        u64::from_le_bytes(tail[40..48].try_into().unwrap())
    }

    #[test]
    fn mgba_to_gambatte_subtracts_the_latched_time() {
        let tail = mgba_tail([0; 5], [5, 4, 3, 300 & 0xFF, 1], 1_800_000_000);
        let out = convert(&tail, GAMBATTE, 1_900_000_000).expect("no conversion");
        assert_eq!(
            u64::from_le_bytes(out[..].try_into().unwrap()),
            1_800_000_000 - (5 + 240 + 10_800 + 300 * 86_400)
        );
    }

    #[test]
    fn gambatte_to_mgba_splits_elapsed_time() {
        let now = 1_800_000_000u64;
        let base = (now - 90_061).to_le_bytes();
        let out = convert(&base, MGBA, now).expect("no conversion");
        assert_eq!(out.len(), MGBA);
        assert_eq!(registers(&out, 0), [1, 1, 1, 1, 0]);
        assert_eq!(registers(&out, 20), [1, 1, 1, 1, 0]);
        assert_eq!(unix(&out), now);
    }

    #[test]
    fn gambatte_to_mgba_wraps_past_511_days_like_gambatte() {
        let now = 1_800_000_000u64;
        let at = |days: u64, extra: u64| {
            let base = (now - days * 86_400 - extra).to_le_bytes();
            convert(&base, MGBA, now).expect("no conversion")
        };
        assert_eq!(registers(&at(600, 0), 0), [0, 0, 0, 89, 0]);
        assert_eq!(registers(&at(600, 0), 20), [0, 0, 0, 89, 0]);
        assert_eq!(registers(&at(511, 0), 0), [0, 0, 0, 255, 1]);
        assert_eq!(registers(&at(512, 0), 0), [0, 0, 0, 1, 0]);
        assert_eq!(registers(&at(600, 3_661), 0), [1, 1, 1, 89, 0]);
        assert_eq!(unix(&at(600, 0)), now);
    }

    #[test]
    fn mgba_round_trips_through_gambatte() {
        let now = 1_800_000_000u64;
        let tail = mgba_tail([9; 5], [5, 4, 3, 5, 0], now - 100);
        let gambatte = convert(&tail, GAMBATTE, now).expect("no conversion");
        let back = convert(&gambatte, MGBA, now).expect("no conversion back");
        assert_eq!(registers(&back, 20), [45, 5, 3, 5, 0]);
        assert_eq!(registers(&back, 0), [45, 5, 3, 5, 0]);
        assert_eq!(unix(&back), now);
    }

    #[test]
    fn other_sizes_are_left_alone() {
        assert_eq!(convert(&[0; MGBA], MGBA, 0), None);
        assert_eq!(convert(&[0; GAMBATTE], GAMBATTE, 0), None);
        assert_eq!(convert(&[0; 12], GAMBATTE, 0), None);
        assert_eq!(convert(&[0; MGBA], 16, 0), None);
        assert_eq!(convert(&[0; GAMBATTE], 16, 0), None);
        assert_eq!(convert(&[], MGBA, 0), None);
    }
}
