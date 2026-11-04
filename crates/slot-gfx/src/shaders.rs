use crate::surface::GfxError;

const VERT_PREAMBLE: &str = "#version 330 core\n#define attribute in\n#define varying out\n";

const FRAG_PREAMBLE: &str = "#version 330 core\n#define varying in\n\
                             #define texture2D texture\nout vec4 FRAG_COLOR;\n";

const VERT_PREAMBLE_ES: &str = "";
const FRAG_PREAMBLE_ES: &str = "#define FRAG_COLOR gl_FragColor\n";

pub fn program(vert: &str, frag: &str) -> Result<gl::types::GLuint, GfxError> {
    let (vp, fp) = match crate::gl::es() {
        true => (VERT_PREAMBLE_ES, FRAG_PREAMBLE_ES),
        false => (VERT_PREAMBLE, FRAG_PREAMBLE),
    };
    crate::gl::program(&format!("{vp}{vert}"), &format!("{fp}{frag}"))
}

pub const RECT_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 p = (u_rect.xy + a_pos * u_rect.zw) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

pub const SPRITE_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
uniform vec4 u_src;
uniform float u_quad;
uniform vec4 u_c01;
uniform vec4 u_c23;
varying vec2 v_uv;
void main() {
    v_uv = u_src.xy + a_pos * u_src.zw;
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_pos * u_rect.zw - mid;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    vec2 flat_p = u_rect.xy + a_pos * u_rect.zw + (turned - local);
    vec2 top = mix(u_c01.xy, u_c01.zw, a_pos.x);
    vec2 bottom = mix(u_c23.zw, u_c23.xy, a_pos.x);
    vec2 quad_p = mix(top, bottom, a_pos.y);
    vec2 p = mix(flat_p, quad_p, u_quad) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

pub const GAME_FRAG: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_game;
uniform sampler2D u_paper;
uniform vec2 u_src;
uniform vec4 u_uv;
uniform float u_bright;
uniform float u_mode;
uniform float u_paper_size;
uniform vec2 u_origin;
uniform vec4 u_rect;
uniform vec2 u_fbo;
uniform vec4 u_pic;
uniform sampler2D u_ramp;
uniform vec3 u_light;
uniform vec3 u_dark;
uniform mat3 u_response;
uniform vec3 u_gamma;
uniform vec3 u_least;
uniform float u_grain;
uniform vec4 u_lcd;
uniform vec4 u_ink;
uniform vec3 u_glass;
uniform float u_sheen;
varying vec2 v_uv;
const vec3 offsets = vec3(3.141592654) * vec3(1.0 / 2.0, 1.0 / 2.0 - 2.0 / 3.0, 1.0 / 2.0 - 4.0 / 3.0);
const vec3 luma_coeff = vec3(0.2126, 0.7152, 0.0722);
vec2 grid_band(vec2 x) {
    return floor(x) / 3.0 + min(fract(x), vec2(1.0 / 3.0));
}
float dot_weight(vec2 f, vec2 o, float bloom) {
    return exp(-4.0 * length(f - (o + vec2(0.5))) * bloom);
}
// A Game Boy dot's ink, from the grey the core draws: dmg-3x's alpha.
float ink(vec2 at) {
    return min(u_ink.x * (1.0 - dot(texture2D(u_game, at).rgb, vec3(1.0 / 3.0))) + u_ink.y, 1.0);
}
// The ink anywhere on the screen, filtered between game pixels, and none past the picture.
float ink_near(vec2 at, vec2 texel) {
    vec2 lo = u_pic.xy + 0.5 * texel;
    vec2 hi = u_pic.zw - 0.5 * texel;
    vec2 cover = clamp(1.0 - max(lo - at, at - hi) * u_src, 0.0, 1.0);
    return ink(clamp(at, lo, hi)) * cover.x * cover.y;
}
void main() {
    vec2 uv = u_uv.xy + v_uv * u_uv.zw;
    vec2 f = fract(uv * u_src);
    vec3 c = texture2D(u_game, uv).rgb;
    bool inside = all(greaterThanEqual(uv, u_pic.xy)) && all(lessThan(uv, u_pic.zw));
    if (u_mode > 5.5) {
        // Authentic: an unlit LCD in room light, its panel running on past the picture to
        // the edge of the screen. See authentic.rs.
        vec2 texel = 1.0 / u_src;
        vec2 centre = (floor(uv * u_src) + 0.5) * texel;
        vec2 span = u_rect.zw * u_fbo / (u_uv.zw * u_src);
        // The screen pixel along each game pixel's top and left edge is the gap between dots.
        vec2 gap = step(f * span, vec2(1.0));
        if (u_lcd.x > 0.5) {
            // The core draws grey: how much ink each dot holds over the lit panel. A gap
            // takes a share of the ink on either side of it, and none where gaps cross.
            float a = 0.0;
            if (inside) {
                a = ink(centre);
                if (gap.x + gap.y > 0.5) {
                    vec2 back = centre - (gap.x > 0.5 ? vec2(texel.x, 0.0) : vec2(0.0, texel.y));
                    float other = all(greaterThanEqual(back, u_pic.xy)) ? ink(back) : 0.0;
                    a = gap.x + gap.y > 1.5 ? 0.0 : u_ink.z * (a + other);
                }
            }
            // The dots' shadow on the reflector, a game pixel down and to the left (dmg-3x
            // casts it half as far), blurred over about a pixel. Ink in front of it hides it.
            vec2 from = uv + vec2(1.0, -1.0) * texel;
            float shadow = ink_near(from - 0.5 * texel, texel) + ink_near(from + 0.5 * texel, texel)
                + ink_near(from + vec2(0.5, -0.5) * texel, texel) + ink_near(from + vec2(-0.5, 0.5) * texel, texel);
            shadow *= 0.25 * u_ink.w * (1.0 - 0.75 * a);
            c = u_dark * a * (1.0 - u_glass.x * a * a) + u_light * (1.0 - shadow) * (u_glass.y - u_glass.z * a);
        } else if (!inside) {
            c = pow(u_light, vec3(1.0 / 2.2));
        } else {
            // Each 5-bit code's measured light, then the panel's colour response, then the
            // reflector's white and the panel's black.
            c = texture2D(u_game, centre).rgb;
            // The 5-bit code is each channel's top 5 bits, however the frame widened it to 8.
            vec3 code = (floor((c * 255.0 + 0.5) / 8.0) + 0.5) / 32.0;
            vec2 pick = vec2(1.0 - u_lcd.y, u_lcd.y);
            vec3 light = vec3(dot(texture2D(u_ramp, vec2(code.r, 0.5)).rg, pick),
                              dot(texture2D(u_ramp, vec2(code.g, 0.5)).rg, pick),
                              dot(texture2D(u_ramp, vec2(code.b, 0.5)).rg, pick));
            vec3 lin = clamp(u_response * pow(light, u_gamma), u_least, vec3(1.0));
            lin = mix(u_dark, u_light, lin);
            // The measured colour is a whole pixel's: its cell and the dark grid around it.
            // Light the cell so that the two together still average to it.
            vec2 thin = min(1.0 / span, vec2(1.0));
            float grid = thin.x * (1.0 - thin.y) * u_lcd.z + thin.y * (1.0 - thin.x) * u_lcd.w
                + thin.x * thin.y * max(u_lcd.z, u_lcd.w);
            // The reflector shows through the colour filter as a fine sparkle.
            float speck = fract(sin(dot(floor(gl_FragCoord.xy), vec2(12.9898, 78.233))) * 43758.5453);
            float sparkle = 1.0 + u_grain * (step(0.9, speck) * 10.0 - 1.0);
            lin = min(u_dark + (lin - u_dark) * sparkle / (1.0 - grid), vec3(1.0));
            lin = mix(lin, u_dark, max(gap.x * u_lcd.z, gap.y * u_lcd.w));
            c = pow(lin, vec3(1.0 / 2.2));
        }
        // The glass, as dmg-3x's border lays it over the screen: the room's light across the
        // top right, and the shade of the bezel toward the bottom.
        float sheen = 0.19 * max(v_uv.x - 0.14 - 0.87 * v_uv.y, 0.0);
        float shade = max(v_uv.y - 0.55 - 0.05 * v_uv.x, 0.0) * (0.44 - 0.11 * v_uv.x);
        c = mix(c, vec3(1.0), sheen * u_sheen) * (1.0 - shade * u_sheen);
    } else if (!inside) {
        // Past the picture, the source shows as it is.
    } else if (u_mode > 4.5) {
        vec2 hp = 0.5 * u_uv.zw * u_src / (u_rect.zw * u_fbo);
        vec2 p = uv * u_src;
        vec2 b = floor(p + hp);
        vec2 t = clamp((p + hp - b) / (2.0 * hp), 0.0, 1.0);
        vec2 lo = u_pic.xy * u_src + 0.5;
        vec2 hi = u_pic.zw * u_src - 0.5;
        vec2 a0 = clamp(b - 0.5, lo, hi) / u_src;
        vec2 a1 = clamp(b + 0.5, lo, hi) / u_src;
        vec3 top = mix(texture2D(u_game, a0).rgb, texture2D(u_game, vec2(a1.x, a0.y)).rgb, t.x);
        vec3 bottom = mix(texture2D(u_game, vec2(a0.x, a1.y)).rgb, texture2D(u_game, a1).rgb, t.x);
        c = mix(top, bottom, t.y);
    } else if (u_mode > 3.5) {
        c = pow(c, vec3(1.05));
        vec2 d = abs(f - 0.5);
        float x2 = max(d.x, d.y);
        x2 = x2 * x2;
        float x4 = x2 * x2;
        float w = 48.0 * (x4 - (8.0 / 3.0) * x4 * x2);
        w = w * (w + (1.0 - w) * 0.65) * 0.65;
        float luma = dot(c, luma_coeff);
        w = w * luma;
        c = mix(c, vec3(1.0), w);
        vec3 bg = vec3(texture2D(u_paper, (floor(gl_FragCoord.xy - u_origin) + 0.5) / u_paper_size).r) * c;
        c = mix(c, bg, dot(c, luma_coeff));
    } else if (u_mode > 2.5) {
        float bloom = mix(1.05, 0.95, dot(c, vec3(0.30, 0.59, 0.11)));
        float mid = dot_weight(f, vec2(0.0), bloom);
        float sum = 0.0;
        for (int j = -1; j <= 1; j++) {
            for (int i = -1; i <= 1; i++) {
                sum += dot_weight(f, vec2(float(i), float(j)), bloom);
            }
        }
        c = mix(c, mix(1.2 * c * mid, c * sum, 0.85), 0.85);
    } else if (u_mode > 1.5) {
        vec2 hp = 0.5 * u_uv.zw * u_src / (u_rect.zw * u_fbo);
        vec2 edge = (grid_band(f + hp) - grid_band(f - hp)) / (2.0 * hp);
        c *= 1.0 - 0.75 * max(edge.x, edge.y);
    } else if (u_mode > 0.5) {
        vec2 angle = f * 6.283185307;
        float yfactor = (16.0 + sin(angle.y)) / 17.0;
        vec3 xfactors = (4.0 + sin(angle.x + offsets)) / 5.0;
        c *= yfactor * xfactors;
    }
    FRAG_COLOR = vec4(c * u_bright, 1.0);
}
"#;

pub const SPRITE_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec4 u_colour;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = texture2D(u_tex, v_uv) * u_colour;
}
"#;

pub const PAPER_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
varying vec2 P;
void main() {
    P = u_rect.xy + a_pos * u_rect.zw;
    vec2 p = P / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

pub const PAPER_FRAG: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_tex;
uniform vec2 pv;
uniform float sc, lift, tear, fold, mode, bow;
varying vec2 P;
const vec2 SZ = vec2(226.0, 425.0);
const vec2 D = vec2(-0.62, -0.78);
const float CUTY = 407.0;
const float TH = 0.42;
const float SQ = 1.5;
const float R = 24.0;
const float PI = 3.14159265;
vec2 local(vec2 s) {
    return (s - pv) / sc + SZ;
}
float alphaAt(vec2 q) {
    if (q.x < 0.0 || q.y < 0.0 || q.x > SZ.x || q.y > SZ.y) return 0.0;
    if (q.x < tear) {
        float saw = abs(fract(q.x / 6.0) - 0.5) * 5.0;
        if (q.y > CUTY - saw) return 0.0;
    }
    return texture2D(u_tex, q / SZ).a;
}
vec4 front(vec2 q) {
    if (alphaAt(q) <= 0.0) return vec4(0.0);
    vec4 t = texture2D(u_tex, q / SZ);
    t.rgb /= t.a;
    return t;
}
bool inside(vec2 q) {
    return alphaAt(q) > 0.5;
}
vec4 sheet(vec2 Q) {
    float d = dot(Q, D) + fold * dot(D, D);
    vec2 B = Q - D * d;
    vec3 backc = vec3(0.885, 0.895, 0.88);
    if (mode > 0.5) {
        if (d < R) {
            float s = d < 0.0 ? PI * R - d * SQ : R * (PI - asin(d / R));
            vec2 q = B + D * s;
            if (inside(q)) {
                float nz = d < 0.0 ? 1.0 : sqrt(max(0.0, 1.0 - d * d / (R * R)));
                float l = mix(0.70, 0.97, nz);
                float ghost = 1.0 - 0.07 * (1.0 - dot(front(q).rgb, vec3(0.333)));
                return vec4(backc * l * ghost, 1.0);
            }
        }
        return vec4(0.0);
    }
    if (d >= 0.0 && d < R) {
        float s = R * asin(d / R);
        vec2 q = B + D * s;
        if (inside(q)) {
            float nz = sqrt(max(0.0, 1.0 - d * d / (R * R)));
            vec4 t = front(q);
            t.rgb *= mix(0.74, 1.0, nz);
            return t;
        }
    }
    if (d < 0.0) {
        vec4 t = front(Q);
        if (t.a > 0.0) {
            t.rgb *= 1.0 - 0.20 * exp(d / (R * 0.9));
            return t;
        }
    }
    return vec4(0.0);
}
float cover(vec2 Q) {
    float d = dot(Q, D) + fold * dot(D, D);
    if (d >= R) return 0.0;
    vec2 B = Q - D * d;
    if (mode > 0.5) {
        float s = d < 0.0 ? PI * R - d * SQ : R * (PI - asin(d / R));
        return inside(B + D * s) ? 1.0 : 0.0;
    }
    if (d >= 0.0) {
        float a = alphaAt(B + D * (R * asin(d / R)));
        return a > 0.5 ? a : 0.0;
    }
    return alphaAt(Q);
}
vec2 unbow(vec2 Q) {
    if (bow <= 0.0) return Q;
    float u = (Q.y - CUTY * 0.5) / (CUTY * 0.5) * (TH / sin(TH));
    float su = u * sin(TH);
    if (abs(su) >= 1.0) return vec2(-1.0);
    float f = asin(su) / TH;
    float w = 1.0 + 0.022 * sqrt(1.0 - su * su);
    return vec2(113.0 + (Q.x - 113.0) / w, CUTY * 0.5 + f * CUTY * 0.5);
}
void main() {
    vec2 Q = unbow(local(P));
    vec4 s = sheet(Q);
    if (bow > 0.0 && s.a > 0.0) {
        float n = clamp((Q.y - CUTY * 0.5) / (CUTY * 0.5) * sin(TH), -1.0, 1.0);
        s.rgb *= 1.0 - (0.13 * max(n, 0.0) + 0.03 * max(-n, 0.0)) + 0.02 * (1.0 - abs(n));
        s.rgb *= 1.0 - 0.10 * pow(abs(n) / sin(TH), 5.0);
    }
    float aa = s.a;
    float sh = 0.0;
    if (aa < 1.0) {
        for (int i = 0; i < 8; i++) {
            float a = float(i) * 0.785;
            vec2 o = vec2(cos(a), sin(a)) * lift * 0.55 + vec2(lift * 0.3, lift * 0.42);
            sh += cover(unbow(local(P - o))) / 8.0;
        }
    }
    float al = aa + (1.0 - aa) * sh * 0.5;
    FRAG_COLOR = vec4(s.rgb * aa, al);
}
"#;

pub const BLIT_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_uv;
varying vec2 v_uv;
void main() {
    v_uv = u_uv.xy + a_pos * u_uv.zw;
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const TURN_VERT: &str = r#"
attribute vec2 a_pos;
uniform float u_ccw;
varying vec2 v_uv;
void main() {
    vec2 cw = vec2(1.0 - a_pos.y, a_pos.x);
    vec2 ccw = vec2(a_pos.y, 1.0 - a_pos.x);
    v_uv = mix(cw, ccw, u_ccw);
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const BLIT_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec3 u_gain;
uniform float u_saturation;
uniform float u_contrast;
varying vec2 v_uv;
void main() {
    vec3 c = texture2D(u_tex, v_uv).rgb;
    c = mix(vec3(dot(c, vec3(0.2126, 0.7152, 0.0722))), c, 1.0 + u_saturation);
    c = (c - 0.5) * (1.0 + u_contrast) + 0.5;
    FRAG_COLOR = vec4(clamp(c, 0.0, 1.0) * u_gain, 1.0);
}
"#;

/// The LCD's slow response: each channel moves from what the screen showed toward the new
/// frame, more slowly while it turns lighter than while it turns darker.
pub const GHOST_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform sampler2D u_prev;
uniform vec2 u_keep;
varying vec2 v_uv;
void main() {
    vec3 now = texture2D(u_tex, v_uv).rgb;
    vec3 was = texture2D(u_prev, v_uv).rgb;
    vec3 keep = mix(vec3(u_keep.y), vec3(u_keep.x), step(was, now));
    vec3 c = mix(now, was, keep);
    // Eight bits cannot hold the last steps of the fade; finish them.
    c = mix(c, now, step(abs(now - was), vec3(1.5 / 255.0)));
    FRAG_COLOR = vec4(c, 1.0);
}
"#;
