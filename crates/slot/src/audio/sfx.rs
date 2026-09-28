const INSERT: &[u8] = include_bytes!("../../assets/insert.pcm");
const EJECT: &[u8] = include_bytes!("../../assets/eject.pcm");
/// Original, softly enveloped two-note chime, 240 ms at 48 kHz.
const ACHIEVEMENT: &[u8] = include_bytes!("../../assets/achievement.pcm");
const ASSET_HZ: f32 = 48_000.0;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sfx {
    Insert,
    Eject,
    Achievement,
}

impl Sfx {
    pub fn lead(self) -> f32 {
        match self {
            Sfx::Insert => 0.097,
            Sfx::Eject => 0.021,
            Sfx::Achievement => 0.0,
        }
    }

    pub fn tail(self) -> f32 {
        let pcm = match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            Sfx::Achievement => ACHIEVEMENT,
        };
        pcm.len() as f32 / 2.0 / ASSET_HZ - self.lead()
    }

    pub fn render(self, sample_rate: u32) -> Vec<i16> {
        let pcm = match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            Sfx::Achievement => ACHIEVEMENT,
        };
        let mut out = Vec::with_capacity(pcm.len());
        for v in resampled(pcm, sample_rate) {
            let s = v.clamp(-32768.0, 32767.0) as i16;
            out.push(s);
            out.push(s);
        }
        out
    }
}

fn resampled(pcm: &[u8], sample_rate: u32) -> Vec<f32> {
    let src: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32)
        .collect();
    if sample_rate == ASSET_HZ as u32 || src.is_empty() {
        return src;
    }
    let ratio = sample_rate as f32 / ASSET_HZ;
    let n = (src.len() as f32 * ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f32 / ratio;
            let a = x as usize;
            let f = x - a as f32;
            let b = (a + 1).min(src.len() - 1);
            src[a] * (1.0 - f) + src[b] * f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn achievement_chime_is_brief_quiet_and_fades_at_both_ends() {
        for rate in [32_768, 48_000] {
            let samples = Sfx::Achievement.render(rate);
            let seconds = samples.len() as f32 / (rate * 2) as f32;
            assert!((0.23..0.25).contains(&seconds));
            let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!((1_000..5_000).contains(&peak));
            assert!(samples[..8].iter().all(|s| s.unsigned_abs() < 100));
            assert!(samples[samples.len() - 100..]
                .iter()
                .all(|s| s.unsigned_abs() < 100));
            let mut muted = samples;
            crate::audio::volume::apply(&mut muted, 0);
            assert!(muted.iter().all(|s| *s == 0));
        }
    }
}
