import base64, io, json, pathlib, shutil, subprocess, tempfile, time, urllib.request

import websocket
from PIL import Image

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
PORT = 9337
HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent.parent / "assets" / "romm"
LAYERS = {
    "box_body": (296, 154),
    "box_inside": (296, 36),
    "lid": (322, 108),
    "glow": (440, 272),
}

profile = tempfile.mkdtemp()
chrome = subprocess.Popen(
    [
        CHROME,
        "--headless=new",
        f"--remote-debugging-port={PORT}",
        "--remote-allow-origins=*",
        "--allow-file-access-from-files",
        f"--user-data-dir={profile}",
        "about:blank",
    ],
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
try:
    for _ in range(50):
        try:
            urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json/version")
            break
        except OSError:
            time.sleep(0.2)
    made = urllib.request.Request(f"http://127.0.0.1:{PORT}/json/new", method="PUT")
    tab = json.load(urllib.request.urlopen(made))
    ws = websocket.create_connection(tab["webSocketDebuggerUrl"])
    sent = [0]

    def call(method, **params):
        sent[0] += 1
        ws.send(json.dumps({"id": sent[0], "method": method, "params": params}))
        while True:
            reply = json.loads(ws.recv())
            if reply.get("id") == sent[0]:
                return reply["result"]

    call("Page.enable")
    call("Emulation.setDeviceMetricsOverride", width=720, height=480, deviceScaleFactor=1, mobile=False)
    call("Emulation.setDefaultBackgroundColorOverride", color={"r": 0, "g": 0, "b": 0, "a": 0})
    OUT.mkdir(exist_ok=True)
    for name, size in LAYERS.items():
        call("Page.navigate", url=f"{(HERE / 'unwrap.html').as_uri()}?layer={name}")
        for _ in range(100):
            title = call("Runtime.evaluate", expression="document.title")
            if title["result"].get("value") == "done":
                break
            time.sleep(0.05)
        shot = call("Page.captureScreenshot", format="png")
        png = Image.open(io.BytesIO(base64.b64decode(shot["data"]))).convert("RGBA")
        png.crop((0, 0, *size)).save(OUT / f"{name}.png", optimize=True)
finally:
    chrome.kill()
    shutil.rmtree(profile, ignore_errors=True)
