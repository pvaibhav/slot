import base64
import json
import math
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets"
OUT = ROOT / ".github" / "resources" / "badges"
FONT = TTFont(ROOT / "crates" / "slot-ui" / "assets" / "label.ttf")
GLYPHS = FONT.getGlyphSet()
CMAP = FONT.getBestCmap()
UPM = FONT["head"].unitsPerEm

CARTS = {
    "gba": {"w": 140, "src": 360, "h": 212, "label": (0.136, 0.242, 0.726, 0.63)},
}

BADGES = [
    {"name": "release", "case": "all", "cart": "gba", "shell": "#7c5ce0", "caption": "download", "value": "v1.15.3"},
    {"name": "guide", "case": "all", "cart": "gba", "shell": "#b8202f", "caption": "read the", "value": "guide"},
    {"name": "discord", "case": "all", "cart": "gba", "shell": "#2f5cc0", "caption": "chat on", "value": "discord"},
    {"name": "stars", "case": "all", "cart": "gba", "shell": "#f2c230", "caption": "stars", "value": "325"},
    {"name": "downloads", "case": "all", "cart": "gba", "shell": "#1e9c5a", "caption": "downloads", "value": "11.2k"},
]

CAPTION_SIZE = 8.5
CAPTION_TRACK = 0.16
VALUE_SIZE = 21
VALUE_TRACK = -0.04
GAP = 5


def glyph_run(text, size, track):
    scale = size / UPM
    x = 0.0
    parts = []
    for i, ch in enumerate(text):
        name = CMAP[ord(ch)]
        pen = SVGPathPen(GLYPHS)
        GLYPHS[name].draw(TransformPen(pen, (scale, 0, 0, -scale, x, 0)))
        d = pen.getCommands()
        if d:
            parts.append(d)
        x += GLYPHS[name].width * scale
        if i < len(text) - 1:
            x += track * size
    return " ".join(parts), x


def baseline(size):
    hhea = FONT["hhea"]
    asc = hhea.ascent / UPM
    desc = -hhea.descent / UPM
    return size * (asc - (asc + desc - 1) / 2)


def shrink(src, width):
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "s.png"
        subprocess.run([
            "magick", str(src), "-resize", f"{width * 2}x>", "-depth", "8", "-strip", str(out),
        ], check=True)
        subprocess.run(["pngquant", "--force", "--quality", "70-95", "--speed", "1", "--ext", ".png", str(out)], check=True)
        return out.read_bytes()


CLEAR_ALPHA = 204 / 255


def overlay(base, colour):
    hi = colour + (255 - colour) * (base - 128) / 127
    lo = colour - colour * (128 - base) / 128
    return np.clip(np.where(base >= 128, hi, lo), 0, 255)


def layer(name, colour, alpha):
    px = np.asarray(Image.open(ASSETS / name).convert("RGBA"), dtype=np.float64)
    if colour is not None:
        px[..., :3] = overlay(px[..., :3], colour)
    px[..., 3] *= alpha
    return px


def over(top, bottom):
    ta = top[..., 3:4] / 255
    ba = bottom[..., 3:4] / 255
    a = ta + ba * (1 - ta)
    rgb = (top[..., :3] * ta + bottom[..., :3] * ba * (1 - ta)) / np.maximum(a, 1e-6)
    return np.concatenate([rgb, a * 255], axis=-1)


LAYERS = {
    "gba": ("gba_interior.png", "board.png", "gba_clear.png"),
}


def tinted(cart, shell, width):
    colour = np.array([int(shell[i:i + 2], 16) for i in (1, 3, 5)], dtype=np.float64)
    back, board, front = LAYERS[cart]
    shape = layer(front, None, 1)[..., 3:4] / 255
    px = layer(back, colour, CLEAR_ALPHA)
    px[..., 3:4] *= shape
    pcb = layer(board, None, 1)
    pcb[..., 3:4] *= shape
    px = over(pcb, px)
    px = over(layer(front, colour, CLEAR_ALPHA), px)
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "t.png"
        Image.fromarray(np.round(px).astype(np.uint8), "RGBA").save(out)
        return shrink(out, width)


def image_size(png):
    return int.from_bytes(png[16:20], "big"), int.from_bytes(png[20:24], "big")


GUT = 12
CAP = 10
TOP = 14
WELL = 4
NOTCH = 18
NOTCH_DEPTH = 8
BELOW = 22
RADIUS = 17.5
HERO_W = 960
CASE_W = 840
TEXTURE = 256 * CASE_W / HERO_W
STRIP = 58 * 2 / 3 * CASE_W / HERO_W


def plastic():
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "p.jpg"
        subprocess.run([
            "magick", str(ROOT / "site" / "docs" / "assets" / "footer-plastic.png"),
            "-quality", "82", "-strip", str(out),
        ], check=True)
        return "data:image/jpeg;base64," + base64.b64encode(out.read_bytes()).decode()


PLASTIC = plastic()


def cart_svg(png, w, h, label, body):
    lx, ly, lw, lh = label
    data = base64.b64encode(png).decode()
    return (
        f'<g filter="url(#s)"><image width="{w:.2f}" height="{h:.2f}" '
        f'xlink:href="data:image/png;base64,{data}" href="data:image/png;base64,{data}"/></g>'
        f'<rect x="{lx:.2f}" y="{ly:.2f}" width="{lw:.2f}" height="{lh:.2f}" rx="3" fill="url(#l)"/>'
        f'<rect x="{lx + .5:.2f}" y="{ly + .5:.2f}" width="{lw - 1:.2f}" height="{lh - 1:.2f}" rx="2.5" '
        'fill="none" stroke="#fff" stroke-opacity=".06"/>'
        f'{body}'
    )


def outline(tw, hh, first, last):
    l = RADIUS if first else 0
    r = RADIUS if last else 0
    return (
        f"M0,0 H{tw} V{hh - r} A{r},{r} 0 0 1 {tw - r},{hh} "
        f"H{l} A{l},{l} 0 0 1 0,{hh - l} Z"
    )


def well(x, y, w, h):
    r = 7
    x0, y0, x1, y1 = x - WELL, y - WELL, x + w + WELL, y + h + WELL
    cx = (x0 + x1) / 2
    return (
        f"M{x0 + r},{y0} H{x1 - r} A{r},{r} 0 0 1 {x1},{y0 + r} V{y1 - r} A{r},{r} 0 0 1 {x1 - r},{y1} "
        f"H{cx + NOTCH} A{NOTCH},{NOTCH_DEPTH} 0 0 1 {cx - NOTCH},{y1} "
        f"H{x0 + r} A{r},{r} 0 0 1 {x0},{y1 - r} V{y0 + r} A{r},{r} 0 0 1 {x0 + r},{y0} Z"
    )


def tile(inner, w, h, gx, first, last, tall):
    hh = TOP + tall + BELOW
    left = GUT + (CAP if first else 0)
    tw = left + w + GUT + (CAP if last else 0)
    cy = TOP + tall - h
    pocket = well(left, cy, w, h)
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
        f'width="{tw:.0f}" height="{hh}" viewBox="0 0 {tw:.2f} {hh}">'
        '<defs>'
        '<filter id="s" x="-20%" y="-20%" width="140%" height="150%">'
        '<feDropShadow dx="0" dy="2" stdDeviation="1.6" flood-color="#000" flood-opacity=".7"/></filter>'
        '<filter id="i" x="-10%" y="-10%" width="120%" height="120%">'
        '<feGaussianBlur in="SourceAlpha" stdDeviation="2.4"/><feOffset dy="3"/>'
        '<feComposite in2="SourceAlpha" operator="arithmetic" k2="-1" k3="1"/>'
        '<feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 .85 0"/></filter>'
        '<linearGradient id="l" x1="0" y1="0" x2=".34" y2="1">'
        '<stop offset="0" stop-color="#2a2a31"/><stop offset="1" stop-color="#121216"/></linearGradient>'
        f'<pattern id="p" patternUnits="userSpaceOnUse" width="{TEXTURE:.2f}" height="{TEXTURE:.2f}" '
        f'x="{-gx:.2f}" y="{-STRIP:.2f}"><image width="{TEXTURE:.2f}" height="{TEXTURE:.2f}" '
        f'xlink:href="{PLASTIC}" href="{PLASTIC}"/></pattern>'
        '<linearGradient id="k" x1="0" y1="0" x2="0" y2="1">'
        '<stop offset=".55" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#fff" stop-opacity=".16"/></linearGradient>'
        f'<clipPath id="c"><path d="{outline(tw, hh, first, last)}"/></clipPath>'
        '</defs>'
        '<g clip-path="url(#c)">'
        f'<rect width="{tw:.2f}" height="{hh}" fill="url(#p)"/>'
        f'<rect y="{hh - 1}" width="{tw:.2f}" height="1" fill="#000" opacity=".5"/>'
        f'<path d="{pocket}" fill="#151518"/>'
        f'<path d="{pocket}" fill="#000" filter="url(#i)"/>'
        f'<path d="{pocket}" fill="none" stroke="url(#k)" stroke-width="1.2"/>'
        f'<g transform="translate({left:.2f} {cy:.2f})">{inner}</g>'
        '</g>'
        '</svg>'
    )
    return svg, tw


def size(b):
    cart = CARTS[b["cart"]]
    w = b.get("w", cart["w"])
    return w, w * cart["h"] / cart["src"]


def layout():
    out = []
    cases = []
    for b in BADGES:
        if not cases or cases[-1][0] != b["case"]:
            cases.append((b["case"], []))
        cases[-1][1].append(b)
    for _, group in cases:
        tall = math.ceil(max(size(b)[1] for b in group))
        gx = 0.0
        for i, b in enumerate(group):
            first, last = i == 0, i == len(group) - 1
            tw = GUT + (CAP if first else 0) + size(b)[0] + GUT + (CAP if last else 0)
            out.append((b, gx, first, last, tall))
            gx += tw
    return out


def badge(b, gx, first, last, tall, value=None, template=False):
    cart = b["cart"]
    w, h = size(b)
    png = tinted(cart, b["shell"], w)
    lx, ly, lw, lh = CARTS[cart]["label"]
    label = (lx * w, ly * h, lw * w, lh * h)
    cap_d, cap_w = glyph_run(b["caption"].upper(), CAPTION_SIZE, CAPTION_TRACK)
    val = value if value is not None else b["value"]
    val_d, val_w = glyph_run(val, VALUE_SIZE, VALUE_TRACK)
    cx = label[0] + label[2] / 2
    top = label[1] + (label[3] - (CAPTION_SIZE + GAP + VALUE_SIZE)) / 2
    cap_y = top + baseline(CAPTION_SIZE)
    val_y = top + CAPTION_SIZE + GAP + baseline(VALUE_SIZE)
    val_x = f"{cx - val_w / 2:.2f}"
    if template:
        val_d, val_x = "@@D@@", "@@X@@"
        kit_cx[b["name"]] = round(cx, 2)
    value_el = f'<path transform="translate({val_x} {val_y:.2f})" fill="#f4f5f9" d="{val_d}"/>'
    if template:
        value_el = f'<g transform="translate({val_x} {val_y:.2f})" fill="#f4f5f9">{val_d}</g>'
    body = (
        f'<path transform="translate({cx - cap_w / 2:.2f} {cap_y:.2f})" fill="#9a9ba8" d="{cap_d}"/>'
        + value_el
    )
    return tile(cart_svg(png, w, h, label, body), w, h, gx, first, last, tall)[0]


kit_cx = {}


def worker_kit():
    kit = {"glyphs": {}, "badges": {}, "cx": kit_cx}
    for ch in "0123456789.kMv":
        d, adv = glyph_run(ch, VALUE_SIZE, 0)
        kit["glyphs"][ch] = {"d": d, "w": round(adv, 3)}
    kit["track"] = VALUE_TRACK * VALUE_SIZE
    for b, gx, first, last, tall in layout():
        if b["name"] not in ("stars", "downloads", "release"):
            continue
        kit["badges"][b["name"]] = badge(b, gx, first, last, tall, value="0", template=True)
    return kit


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.svg"):
        old.unlink()
    for b, gx, first, last, tall in layout():
        (OUT / f"{b['name']}.svg").write_text(badge(b, gx, first, last, tall))
    (ROOT / "tools" / "badges" / "kit.json").write_text(json.dumps(worker_kit()))


main()
