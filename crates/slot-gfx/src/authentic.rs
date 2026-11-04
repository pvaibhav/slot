//! The Authentic shader: each console's unlit, reflective LCD as it looks in room light. The
//! reflector behind the liquid crystal is the brightest the picture gets; a pixel only takes
//! light away from it.
//!
//! Sources:
//! - Game Boy and Pocket: Harlequin's gameboy shader in its dmg-3x and gb-pocket-3x presets
//!   (libretro glsl-shaders, handheld/console-border), settings, palettes, shadow and glass
//!   reflection alike. Each dot is a share of ink over the lit panel; the gaps between dots
//!   take some of their neighbours' ink; the dots cast a soft shadow on the reflector. Its
//!   shades are TWB64's "DMG Ver." and PixelShift's "GB Old" and "GB Pocket" palettes, and,
//!   as BGB's "reality" colours do, its middle shades turn blue-green.
//! - Game Boy Light: TWB64's "Light Ver." lightest and darkest shades, on the Pocket's
//!   panel and settings; nobody has measured the Light.
//! - Ghosting: blended in shades of grey before the colours are applied, as BGB's "DMG
//!   reality" frame blend does (bgb.bircd.org/reality), so a flickered shade between black
//!   and white shows the LCD's own middle shade.
//! - GBC: BGB's "GBC reality" colours at their true, dark reflective brightness, made from
//!   photographs of an 8x8x8 cube of colours under calibrated light. Its white, its black
//!   and the 2,560 colours of its test screen fit Pokefan531's measured ramp, raised to a
//!   power for each channel, and a matrix to within about 5 levels of 255.
//! - GBA colour response: Pokefan531's 2026 spectrophotometer measurements
//!   (github.com/Pokefan531/Handheld-Colorspace-Shaders), per-code ramps and matrices.
//! - GBA panel white tint: the Handheld Color Space Project's reflectance readings. Drawn
//!   at the GBC's brightness and contrast, since the measured 5-6:1 (a 45/0 instrument
//!   reading) reads as fog on a lit screen: white as bright as the GBC's, black a 35th of it.

/// The screen an Authentic picture is drawn on.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Screen {
    /// The original Game Boy's yellow-green panel.
    Dmg,
    /// The Game Boy Pocket's grey-olive panel.
    Pocket,
    /// The Game Boy Light's panel, lit from behind in blue-green.
    Light,
    Gbc,
    Gba,
}

pub(crate) struct Look {
    /// A Game Boy's panel, behind and around the dots, in sRGB; a colour screen's lit
    /// white, linear.
    pub light: [f32; 3],
    /// A Game Boy's ink, in sRGB; a colour screen's black, linear.
    pub dark: [f32; 3],
    /// A colour screen's response, from the measured ramp's linear light to linear sRGB,
    /// column by column for the input's red, green and blue.
    pub response: [f32; 9],
    /// The power that takes each channel's measured ramp to linear light.
    pub gamma: [f32; 3],
    /// How far below black, as a share of the way to white, a colour screen draws a colour
    /// its response puts there.
    pub least: [f32; 3],
    /// How much of a colour screen's reflector sparkles through its colour filters: a tenth
    /// of the screen's pixels this much brighter, the rest a little darker.
    pub grain: f32,
    /// Game Boy (1) or colour (0), which ramp (0 GBA, 1 GBC), and how much the gaps between
    /// columns and rows darken.
    pub lcd: [f32; 4],
    /// A Game Boy dot's ink: how much it gains as its grey darkens, what a white dot still
    /// holds, the share of each neighbour's ink a gap takes, and how dark the dots' shadow
    /// falls.
    pub ink: [f32; 4],
    /// How a Game Boy's ink and panel combine: the ink's contrast, the panel's light, and
    /// how much of it a dot takes away.
    pub glass: [f32; 3],
    /// The share of the last frame a pixel keeps each frame while it turns lighter, and
    /// while it turns darker: the LCD's slow response.
    pub ghost: [f32; 2],
}

/// The share of the last frame kept each frame by a pixel that settles with time constant
/// `ms`, at the Game Boy's 59.73 frames a second.
fn keep(ms: f32) -> f32 {
    (-1000.0 / 59.73 / ms).exp()
}

/// Each 5-bit code's light on the GBA and the GBC, in gamma 2.2: Pokefan531's measured ramps.
pub(crate) const RAMPS: [[u8; 32]; 2] = [
    [
        0, 3, 6, 10, 15, 25, 36, 50, 62, 71, 82, 92, 102, 109, 116, 124, 131, 138, 145, 152, 160,
        168, 176, 184, 193, 202, 211, 218, 229, 240, 249, 255,
    ],
    [
        0, 8, 15, 26, 37, 49, 67, 85, 97, 110, 121, 134, 140, 148, 154, 160, 170, 180, 188, 196,
        202, 207, 212, 217, 222, 228, 232, 237, 246, 252, 254, 255,
    ],
];

/// The ramps as a 32 by 1 RGBA texture: the GBA's in red, the GBC's in green.
pub(crate) fn ramp_texture() -> Vec<u8> {
    (0..32)
        .flat_map(|i| [RAMPS[0][i], RAMPS[1][i], 0, 255])
        .collect()
}

fn rgb(hex: u32) -> [f32; 3] {
    [16, 8, 0].map(|s| ((hex >> s) & 0xff) as f32 / 255.0)
}

fn linear(srgb: [f32; 3]) -> [f32; 3] {
    srgb.map(|c| c.powf(2.2))
}

/// One Game Boy model's settings in Harlequin's shader.
struct Dots {
    panel: u32,
    ink: u32,
    baseline_alpha: f32,
    grey_balance: f32,
    contrast: f32,
    screen_light: f32,
    pixel_opacity: f32,
    shadow_opacity: f32,
}

/// dmg-3x.
const DMG: Dots = Dots {
    panel: 0xa1a100,
    ink: 0x005aaa,
    baseline_alpha: 0.05,
    grey_balance: 2.6,
    contrast: 0.7,
    screen_light: 0.85,
    pixel_opacity: 0.9,
    shadow_opacity: 0.9,
};

/// gb-pocket-3x.
const POCKET: Dots = Dots {
    panel: 0xa6ac84,
    ink: 0x121922,
    baseline_alpha: 0.03,
    grey_balance: 3.5,
    contrast: 0.8,
    screen_light: 0.9,
    pixel_opacity: 0.95,
    shadow_opacity: 0.9,
};

/// The Pocket's settings, with the panel and ink that put its white and black on TWB64's
/// Light shades, #00b581 and #004f3b.
const LIGHT: Dots = Dots {
    panel: 0x00cb90,
    ink: 0x006550,
    ..POCKET
};

/// How much of a dot's ink its shadow carries, for every unit of the model's shadow
/// strength. Measured from dmg-3x drawing Metroid II at 3x: under a black sky the panel
/// keeps 0.72 of its light where gaps cross, and the ink in front of a dot hides most of it.
const SHADOW: f32 = 0.45;

/// The share of each neighbour's ink a gap between dots takes (dmg-3x's neighbour blending).
const GAP: f32 = 0.38;

impl Dots {
    fn look(&self, ghost: [f32; 2]) -> Look {
        Look {
            light: rgb(self.panel),
            dark: rgb(self.ink),
            response: [0.0; 9],
            gamma: [2.2; 3],
            least: [0.0; 3],
            grain: 0.0,
            lcd: [1.0, 0.0, 0.0, 0.0],
            ink: [
                3.0 / self.grey_balance,
                self.baseline_alpha,
                GAP,
                self.contrast * self.shadow_opacity * SHADOW,
            ],
            glass: [
                self.contrast,
                self.screen_light,
                self.contrast * self.pixel_opacity,
            ],
            ghost,
        }
    }
}

/// The relative luminance of a linear colour.
fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The GBC's white, in linear light: BGB's GBC reality white.
fn gbc_white() -> [f32; 3] {
    linear(rgb(0x868c7a))
}

impl Screen {
    pub(crate) fn look(self) -> Look {
        match self {
            // The DMG's STN panel clears slowly and darkens faster (brickboy's 61 and 21 ms);
            // the Pocket's and the Light's answer about twice as fast. Nobody has measured them.
            Screen::Dmg => DMG.look([keep(61.0), keep(21.0)]),
            Screen::Pocket => POCKET.look([keep(30.0), keep(10.0)]),
            Screen::Light => LIGHT.look([keep(30.0), keep(10.0)]),
            // BGB's GBC reality white and black. Its blue lies outside sRGB, and BGB draws
            // the red in it darker than the black level, as the screen's own red is.
            Screen::Gbc => {
                let (white, dark) = (gbc_white(), linear(rgb(0x0e191c)));
                Look {
                    light: white,
                    dark,
                    response: [
                        0.9173, 0.0822, 0.1309, 0.1560, 0.6685, 0.1116, -0.0732, 0.2493, 0.7574,
                    ],
                    gamma: [2.2, 2.2 * 0.775, 2.2 * 0.725],
                    least: [0, 1, 2].map(|i| -dark[i] / (white[i] - dark[i])),
                    // A macro photograph shows a sparkly reflector through the filters, and
                    // an opaque dark grid around the pixels, wider between rows.
                    grain: 0.03,
                    lcd: [0.0, 1.0, 0.6, 0.85],
                    ink: [0.0; 4],
                    glass: [0.0; 3],
                    ghost: [keep(16.0), keep(8.0)],
                }
            }
            // White is the panel's tint at the GBC white's luminance; black is a 35th of
            // white's light, a little tinted the way the panel's black measured.
            Screen::Gba => {
                let tint_white = linear(rgb(0xdef6ff));
                let gain = luminance(gbc_white()) / luminance(tint_white);
                let white = tint_white.map(|c| c * gain);
                let tint = [1.02, 0.97, 1.06];
                let response = [
                    0.8988, 0.1163, 0.1831, 0.2229, 0.6217, 0.2438, -0.1217, 0.2620, 0.5731,
                ];
                Look {
                    light: white,
                    dark: [0, 1, 2].map(|i| white[i] / 35.0 * tint[i]),
                    response: response.map(|v| v * 0.8915),
                    gamma: [2.2; 3],
                    least: [0.0; 3],
                    // The GBA's reflector glints under a lamp rather than showing grain.
                    grain: 0.0,
                    lcd: [0.0, 0.0, 0.45, 0.20],
                    ink: [0.0; 4],
                    glass: [0.0; 3],
                    ghost: [keep(20.0), keep(10.0)],
                }
            }
        }
    }
}

/// What the LCD shows after its slow response: two textures at the game's own size, the
/// last one read while the next is drawn. Tiny next to the screen, and none of it on the CPU.
pub(crate) struct Ghost {
    prog: gl::types::GLuint,
    u_keep: gl::types::GLint,
    tex: [gl::types::GLuint; 2],
    fbo: [gl::types::GLuint; 2],
    shown: usize,
    primed: bool,
}

impl Ghost {
    pub fn new() -> Result<Self, crate::surface::GfxError> {
        use crate::pipeline::{SRC_H, SRC_W};
        let prog = crate::shaders::program(crate::shaders::BLIT_VERT, crate::shaders::GHOST_FRAG)?;
        // Filtered, so the dots' shadow blurs between game pixels; the picture itself reads
        // each pixel at its centre.
        let tex = [0, 1].map(|_| {
            crate::gl::texture(SRC_W, SRC_H, gl::LINEAR, gl::CLAMP_TO_EDGE, gl::RGBA, None)
        });
        let mut fbo = [0; 2];
        unsafe {
            gl::GenFramebuffers(2, fbo.as_mut_ptr());
            for (f, t) in fbo.iter().zip(tex) {
                gl::BindFramebuffer(gl::FRAMEBUFFER, *f);
                gl::FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    t,
                    0,
                );
            }
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_prev"), 1);
            gl::Uniform4f(
                crate::gl::uniform_location(prog, "u_uv"),
                0.0,
                0.0,
                1.0,
                1.0,
            );
        }
        let u_keep = crate::gl::uniform_location(prog, "u_keep");
        Ok(Ghost {
            prog,
            u_keep,
            tex,
            fbo,
            shown: 0,
            primed: false,
        })
    }

    /// Start again from the next frame, with nothing of an older picture in it.
    pub fn reset(&mut self) {
        self.primed = false;
    }

    /// The screen after a new frame from `game`, each pixel `keep`ing part of what it showed.
    pub fn respond(&mut self, game: gl::types::GLuint, keep: [f32; 2], quad: &crate::quad::Quad) {
        use crate::pipeline::{SRC_H, SRC_W};
        let next = 1 - self.shown;
        let keep = if self.primed { keep } else { [0.0; 2] };
        unsafe {
            let mut fb = 0;
            let mut view = [0; 4];
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fb);
            gl::GetIntegerv(gl::VIEWPORT, view.as_mut_ptr());
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo[next]);
            gl::Viewport(0, 0, SRC_W as i32, SRC_H as i32);
            gl::UseProgram(self.prog);
            gl::Uniform2f(self.u_keep, keep[0], keep[1]);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.tex[self.shown]);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, game);
            quad.draw();
            gl::BindFramebuffer(gl::FRAMEBUFFER, fb as gl::types::GLuint);
            gl::Viewport(view[0], view[1], view[2], view[3]);
        }
        self.shown = next;
        self.primed = true;
    }

    /// The screen as it shows now, once a frame has gone through it.
    pub fn texture(&self) -> Option<gl::types::GLuint> {
        self.primed.then_some(self.tex[self.shown])
    }
}

impl Drop for Ghost {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(2, self.fbo.as_ptr());
            gl::DeleteTextures(2, self.tex.as_ptr());
            gl::DeleteProgram(self.prog);
        }
    }
}
