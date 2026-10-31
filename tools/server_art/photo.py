import sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from scipy import ndimage as ndi
import qrcode
S, OUT = sys.argv[1], sys.argv[2]
REPO = "/Users/btk/Developer/slot"
LABEL = f"{REPO}/crates/slot-ui/assets/label.ttf"
BOLD = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
SCALE = 0.545
src = np.asarray(Image.open(f"{S}/ref/Game_Boy_Printer.jpg").convert("RGB"), dtype=np.float32) / 255
H, W, _ = src.shape
paper_white = src.min(axis=2) > 0.93
lab, _n = ndi.label(paper_white)
edge = set(lab[0, :]) | set(lab[:, 0]) | set(lab[:, -1]) | set(lab[-1, :])
edge.discard(0)
out = np.isin(lab, list(edge))
out = ndi.binary_dilation(out, iterations=2)
alpha = ndi.gaussian_filter((~out).astype(np.float32), 1.2)
img = src.copy()
y0, y1, x0, x1 = 788, 856, 380, 600
patch = img[y1 + 10: y1 + 10 + (y1 - y0), x0:x1]
fy = np.clip(np.minimum(np.arange(y1 - y0), (y1 - y0 - 1) - np.arange(y1 - y0)) / 12, 0, 1)[:, None]
fx = np.clip(np.minimum(np.arange(x1 - x0), (x1 - x0 - 1) - np.arange(x1 - x0)) / 16, 0, 1)[None, :]
f = (fy * fx)[..., None]
img[y0:y1, x0:x1] = img[y0:y1, x0:x1] * (1 - f) + patch * f
m = Image.new("L", (W, H), 0)
d = ImageDraw.Draw(m)
d.rounded_rectangle((402, 802, 582, 842), 20, outline=255, width=4)
d.text((492, 822), "slot.", font=ImageFont.truetype(BOLD, 28), fill=255, anchor="mm")
m = ndi.gaussian_filter(np.asarray(m, dtype=np.float32) / 255, 0.8)
hi = np.roll(m, (1, 1), (0, 1)) - np.roll(m, (-1, -1), (0, 1))
img += (hi * -0.10)[..., None]
yy, xx = np.ogrid[:H, :W]
op = (xx > 276) & (xx < 712) & (yy > 448) & (yy < 576)
g = img.mean(axis=2, keepdims=True)
img = np.where(op[..., None], img * 0.1 + g * 0.9, img)
r = np.sqrt((xx - 743.0) ** 2 + (yy - 640.0) ** 2)
led = np.clip(15 - r, 0, 1)[..., None]
core = np.clip(1 - r / 15, 0, 1)[..., None]
ledc = np.dstack([0.55 + 0.45 * core[..., 0], 0.03 + 0.25 * core[..., 0] ** 3, 0.03 + 0.2 * core[..., 0] ** 3])
img = img * (1 - led) + ledc * led
img += ndi.gaussian_filter((r < 14).astype(np.float32), 13)[..., None] * np.array([0.55, 0.03, 0.02], np.float32).reshape(1, 1, 3)
rgba = (np.dstack((np.clip(img, 0, 1), alpha)) * 255).astype(np.uint8)
size = (round(W * SCALE), round(H * SCALE))
art = Image.fromarray(rgba, "RGBA").resize(size, Image.LANCZOS)
K = 4
blade = Image.new("RGBA", (size[0] * K, size[1] * K), (0, 0, 0, 0))
bd = ImageDraw.Draw(blade)
for i in range(0, 39):
    x0 = 153 + i * 6
    bd.polygon([(x0 * K, 295 * K), ((x0 + 3) * K, 284 * K), ((x0 + 6) * K, 295 * K)], fill=(46, 48, 53, 255))
    bd.line([(x0 * K, 295 * K), ((x0 + 3) * K, 284 * K)], fill=(176, 178, 184, 255), width=K)
bd.rectangle((153 * K, 295 * K, 387 * K, 299 * K), fill=(30, 31, 35, 255))
bd.line([(153 * K, 295 * K), (387 * K, 295 * K)], fill=(120, 122, 128, 255), width=K)
blade = blade.resize(size, Image.LANCZOS)
art.alpha_composite(blade)
art.save(f"{OUT}/photo-printer.png")
front = np.zeros((H, W), np.float32)
teeth = Image.new("L", (W, H), 0)
td = ImageDraw.Draw(teeth)
td.rectangle((0, 537, W, H), fill=255)
lum = np.clip(img, 0, 1).mean(axis=2)
yy2 = np.arange(H)[:, None]
tooth_mask = ((yy2 >= 524) & (yy2 < 537) & (lum < 0.42)).astype(np.uint8) * 255
merged = np.maximum(np.asarray(teeth), tooth_mask)
merged = np.asarray(Image.fromarray(merged.astype(np.uint8), "L").resize(size, Image.LANCZOS))
front = np.maximum(merged, np.asarray(blade.getchannel("A")))
over = art.copy()
over.putalpha(Image.fromarray(np.minimum(np.asarray(art.getchannel("A")), front), "L"))
px = -50
bg = Image.open(f"{REPO}/assets/background_default.png").convert("RGBA")
sh = Image.new("L", bg.size, 0)
sh.paste(art.getchannel("A"), (px + 5, 9))
sh = sh.filter(ImageFilter.GaussianBlur(12)).point(lambda v: int(v * 0.65))
bg = Image.composite(Image.new("RGBA", bg.size, (0, 0, 0, 255)), bg, sh)
bg.alpha_composite(art, (px, 0))
b = Image.open(f"{REPO}/assets/buttonlegend_round_b.png").convert("RGBA").resize((24, 24), Image.LANCZOS)
bg.convert("RGB").save(f"{OUT}/anim-bg.png")
wood = Image.open(f"{REPO}/assets/background_default.png").convert("RGB")
wood.save(f"{OUT}/anim-wood.png")
layer2 = Image.new("RGBA", wood.size, (0, 0, 0, 0))
sh2 = Image.new("L", wood.size, 0)
sh2.paste(art.getchannel("A"), (px + 5, 9))
sh2 = sh2.filter(ImageFilter.GaussianBlur(12)).point(lambda v: int(v * 0.65))
layer2.paste(Image.new("RGBA", wood.size, (0, 0, 0, 255)), (0, 0), sh2)
layer2.alpha_composite(art, (px, 0))
layer2.save(f"{OUT}/anim-printer.png")
leg = Image.new("RGBA", wood.size, (0, 0, 0, 0))
leg.alpha_composite(b, (518, 444))
ImageDraw.Draw(leg).text((548, 446), "STOP", font=ImageFont.truetype(LABEL, 17), fill=(244, 245, 249, 255))
leg.save(f"{OUT}/anim-legend.png")
top = Image.new("RGBA", bg.size, (0, 0, 0, 0))
top.alpha_composite(over, (px, 0))
top.save(f"{OUT}/anim-over.png")
q = qrcode.QRCode(version=3, error_correction=qrcode.constants.ERROR_CORRECT_M, border=0, box_size=7)
q.add_data("http://192.168.1.42/?pin=4821")
q.make(fit=True)
code = q.make_image(fill_color=(24, 23, 26), back_color=(236, 232, 221)).convert("RGBA")
PW, PH = 226, 410
rng = np.random.default_rng(3)
ink = np.zeros((PH, PW), np.float32)
mat = np.array(q.get_matrix(), dtype=np.float32)
cell = np.kron(mat, np.ones((7, 7), np.float32))
ox = (PW - cell.shape[1]) // 2
ink[40:40 + cell.shape[0], ox:ox + cell.shape[1]] = cell
low = Image.new("L", (PW // 2, PH // 2), 0)
d = ImageDraw.Draw(low)
y = (40 + cell.shape[0] + 14) // 2
for text, sz, gap in (("SCAN OR GO TO", 9, 13), ("SLOT.LOCAL", 13, 15), ("192.168.1.42", 11, 16), ("PIN  4821", 12, 0)):
    d.text((PW // 4, y), text, font=ImageFont.truetype(LABEL, sz), fill=255, anchor="ma")
    y += gap
dots = (np.asarray(low, dtype=np.float32) > 90).astype(np.float32)
dots = np.kron(dots, np.ones((2, 2), np.float32))
ink[:dots.shape[0], :dots.shape[1]] = np.maximum(ink[:dots.shape[0], :dots.shape[1]], dots)
grid = ((np.arange(PH)[:, None] % 2 == 0) | (np.arange(PW)[None, :] % 2 == 0)).astype(np.float32)
head = 0.95 + 0.05 * ndi.gaussian_filter1d(rng.standard_normal(PW).astype(np.float32), 1.2) * 2.2
band = 0.97 + 0.03 * np.sign(np.sin(np.arange(PH) / 3.7 + rng.uniform(0, 3)))
mottle = 1 + ndi.gaussian_filter(rng.standard_normal((PH, PW)).astype(np.float32), 0.8) * 0.16
press = np.clip(head[None, :] * band[:, None] * mottle * (0.90 + 0.10 * grid), 0, 1)
paper = np.ones((PH, PW, 3), np.float32) * np.array([0.945, 0.955, 0.945], np.float32)
paper *= 1 + ndi.gaussian_filter(rng.standard_normal((PH, PW)).astype(np.float32), 1.1)[..., None] * 0.012
inkc = np.array([0.10, 0.145, 0.125], np.float32)
out_rgb = paper * (1 - (ink * press)[..., None]) + inkc * (ink * press)[..., None]
u = (np.arange(PW) / (PW - 1) - 0.5) * 2
roll = 0.90 + 0.10 * np.cos(u * 1.35)
sheen = 0.075 * np.exp(-((u + 0.34) / 0.20) ** 2)
v = np.arange(PH) / (PH - 1)
endcurl = 1 - 0.09 * np.exp(-(v / 0.055) ** 2) - 0.05 * np.exp(-((1 - v) / 0.045) ** 2)
out_rgb = out_rgb * roll[None, :, None] * endcurl[:, None, None] + sheen[None, :, None]
sheet = Image.fromarray((np.clip(out_rgb, 0, 1) * 255).astype(np.uint8), "RGB").convert("RGBA")
mask = Image.new("L", (PW, PH), 255)
md = ImageDraw.Draw(mask)
for i in range(-1, PW // 6 + 2):
    md.polygon([(i * 6, 0), (i * 6 + 3, 5), (i * 6 + 6, 0)], fill=0)
    md.polygon([(i * 6, PH), (i * 6 + 3, PH - 5), (i * 6 + 6, PH)], fill=0)
sheet.putalpha(mask)
sheet.save(f"{OUT}/anim-strip.png")
