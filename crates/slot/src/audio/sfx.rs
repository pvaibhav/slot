const INSERT: &[u8] = include_bytes!("../../assets/insert.pcm");
const EJECT: &[u8] = include_bytes!("../../assets/eject.pcm");
#[cfg(feature = "nds")]
const DS_INSERT: &[u8] = include_bytes!("../../assets/ds_insert.pcm");
#[cfg(feature = "nds")]
const DS_EJECT: &[u8] = include_bytes!("../../assets/ds_eject.pcm");
const PRINT_BUZZ: &[u8] = include_bytes!("../../../../assets/server/buzz.pcm");
const TEAR: &[u8] = include_bytes!("../../../../assets/server/tear.pcm");
const ASSET_HZ: f32 = 48_000.0;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sfx {
    Insert,
    Eject,
    #[cfg(feature = "nds")]
    DsInsert,
    #[cfg(feature = "nds")]
    DsEject,
    PrintBuzz,
    Tear,
}

impl Sfx {
    pub fn lead(self) -> f32 {
        match self {
            Sfx::Insert => 0.097,
            Sfx::Eject => 0.021,
            #[cfg(feature = "nds")]
            Sfx::DsInsert => 0.541,
            #[cfg(feature = "nds")]
            Sfx::DsEject => 0.011,
            Sfx::PrintBuzz | Sfx::Tear => 0.0,
        }
    }

    fn pcm(self) -> &'static [u8] {
        match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            #[cfg(feature = "nds")]
            Sfx::DsInsert => DS_INSERT,
            #[cfg(feature = "nds")]
            Sfx::DsEject => DS_EJECT,
            Sfx::PrintBuzz => PRINT_BUZZ,
            Sfx::Tear => TEAR,
        }
    }

    fn level(self) -> f32 {
        match self {
            Sfx::Insert | Sfx::Eject => 1.0,
            #[cfg(feature = "nds")]
            Sfx::DsInsert | Sfx::DsEject => 1.0,
            Sfx::PrintBuzz => 0.40,
            Sfx::Tear => 0.45,
        }
    }

    pub fn tail(self) -> f32 {
        self.pcm().len() as f32 / 2.0 / ASSET_HZ - self.lead()
    }

    pub fn render(self, sample_rate: u32) -> Vec<i16> {
        let pcm = self.pcm();
        let level = self.level();
        let mut out = Vec::with_capacity(pcm.len());
        for v in resampled(pcm, sample_rate) {
            let s = (v * level).clamp(-32768.0, 32767.0) as i16;
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
