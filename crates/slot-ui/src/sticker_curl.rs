const STRIP: f32 = 8.0;
const CAMERA: f32 = 800.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub turn_deg: f32,
    pub scale: f32,
}

impl Pose {
    pub const REST: Pose = Pose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        turn_deg: 0.0,
        scale: 1.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    pub corners: [[f32; 2]; 4],
    pub uv: [f32; 4],
    pub back: bool,
    pub z: f32,
}

fn bend(i: usize, flat: f32, remaining: f32, curl_deg: f32) -> f32 {
    let j = i as f32 - flat;
    if j <= 0.0 || remaining <= 0.0 {
        return 0.0;
    }
    curl_deg / remaining * (0.4 + 1.2 * j / remaining) * j.min(1.0)
}

pub fn strips(rect: [f32; 4], flat: f32, curl_deg: f32, pose: &Pose, focus: [f32; 2]) -> Vec<Strip> {
    let [rx, ry, w, h] = rect;
    let n = (w / STRIP).ceil().max(1.0) as usize;
    let f = flat.clamp(0.0, 1.0) * n as f32;
    let remaining = n as f32 - f;
    let (sin, cos) = pose.turn_deg.to_radians().sin_cos();
    let place = |lx: f32, ly: f32, lz: f32| -> [f32; 2] {
        let (sx, sy, sz) = (lx * pose.scale, ly * pose.scale, lz * pose.scale);
        let (tx, ty) = (sx * cos - sy * sin, sx * sin + sy * cos);
        let (x, y, z) = (rx + tx + pose.x, ry + ty + pose.y, sz + pose.z);
        let k = CAMERA / (CAMERA - z).max(1.0);
        [focus[0] + (x - focus[0]) * k, focus[1] + (y - focus[1]) * k]
    };
    let mut out = Vec::with_capacity(n);
    let (mut px, mut pz, mut theta) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        let sw = if i + 1 == n { w - STRIP * (n - 1) as f32 } else { STRIP };
        theta += bend(i, f, remaining, curl_deg).to_radians();
        let (qx, qz) = (px + sw * theta.cos(), pz + sw * theta.sin());
        out.push(Strip {
            corners: [place(px, 0.0, pz), place(qx, 0.0, qz), place(qx, h, qz), place(px, h, pz)],
            uv: [STRIP * i as f32 / w, 0.0, sw / w, 1.0],
            back: theta.cos() < 0.0,
            z: (pz + qz) / 2.0 * pose.scale + pose.z,
        });
        px = qx;
        pz = qz;
    }
    out.sort_by(|a, b| a.z.total_cmp(&b.z));
    out
}
