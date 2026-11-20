import asyncio
import shutil
import subprocess
import tempfile
from pathlib import Path

from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
SITE = ROOT / "site"
PARTIALS = SITE / "overrides" / "partials"
CLIP = SITE / "docs" / "media" / "main.mp4"
OUT = ROOT / ".github" / "resources" / "hero.webp"
WIDTH, HEIGHT, SCALE = 960, 600, 2


def build(work):
    shutil.copytree(SITE / "docs" / "assets", work / "assets")
    frames = work / "heroframes"
    frames.mkdir()
    subprocess.run([
        "ffmpeg", "-v", "error", "-i", str(CLIP),
        "-vf", r"select=not(mod(n\,2))", "-fps_mode", "vfr", str(frames / "%03d.png"),
    ], check=True)
    page = (HERE / "hero.html").read_text()
    page = page.replace("@@SVG@@", (PARTIALS / "device.svg").read_text())
    page = page.replace("@@JS@@", (PARTIALS / "device-depth.js").read_text())
    (work / "hero.html").write_text(page)
    return len(list(frames.glob("*.png")))


async def capture(work, count, shots):
    async with async_playwright() as p:
        browser = await p.chromium.launch(channel="chrome")
        page = await browser.new_page(viewport={"width": WIDTH, "height": HEIGHT}, device_scale_factor=SCALE)
        await page.goto((work / "hero.html").as_uri())
        await page.wait_for_function(
            "document.querySelector('img.screen-box').complete && document.fonts.status == 'loaded'"
        )
        await page.wait_for_timeout(500)
        hero = page.locator(".hx")
        for i in range(1, count + 1):
            await page.evaluate(
                "i => { const v = document.querySelector('img.screen-box');"
                " v.src = 'heroframes/' + String(i).padStart(3, '0') + '.png';"
                " return v.decode().then(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))) }",
                i,
            )
            await hero.screenshot(path=str(shots / f"{i:03d}.png"), omit_background=True)
        await browser.close()


def encode(shots):
    subprocess.run([
        "img2webp", "-loop", "0", "-lossy", "-q", "82", "-m", "4", "-d", "33",
        *sorted(str(p) for p in shots.glob("*.png")), "-o", str(OUT),
    ], check=True)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        count = build(work)
        shots = work / "shots"
        shots.mkdir()
        asyncio.run(capture(work, count, shots))
        encode(shots)
    print(OUT)


main()
