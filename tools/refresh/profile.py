#!/usr/bin/env python3
"""Profile fixed-mode Slot sessions on an RG SP without changing the user's saves."""

import argparse
import csv
import hashlib
import io
import json
import pathlib
import re
import shlex
import statistics
import struct
import subprocess
import time


SCENES = {
    "gba-mgba": ("GBA", "mgba", "Advance Wars (USA) (Rev 1).gba"),
    "gba-gpsp": ("GBA", "gpsp", "Advance Wars (USA) (Rev 1).gba"),
    "gba-heavy-mgba": ("GBA", "mgba", "Mario Tennis - Power Tour (USA, Australia) (En,Fr,De,Es,It).gba"),
    "gba-heavy-gpsp": ("GBA", "gpsp", "Mario Tennis - Power Tour (USA, Australia) (En,Fr,De,Es,It).gba"),
    "gb-mgba": ("GB", "mgba", "Tetris (World) (Rev 1).gb"),
    "gbc-mgba": ("GBC", "mgba", "Mario Tennis (USA).gbc"),
}
MODES = {"60": (False, 0), "120": (True, 0), "runahead": (False, 1),
         "120-runahead": (True, 1)}


def settings(text):
    return dict(line.split("=", 1) for line in text.splitlines() if "=" in line)


def summarize(text):
    rows = list(csv.DictReader(io.StringIO(text)))
    if len(rows) < 3:
        raise RuntimeError("probe returned fewer than three samples")
    start, end = rows[0], rows[-1]
    span = float(end["monotonic_secs"]) - float(start["monotonic_secs"])
    times = [float(r["monotonic_secs"]) for r in rows]
    frames = [int(r["committed_frames"]) for r in rows]
    mean_t, mean_f = statistics.mean(times), statistics.mean(frames)
    fps = sum((t - mean_t) * (f - mean_f) for t, f in zip(times, frames)) / sum(
        (t - mean_t) ** 2 for t in times
    )
    return {
        "seconds": span,
        "game_fps": fps,
        "cpu_percent": (int(end["cpu_ticks"]) - int(start["cpu_ticks"])) / span
        * 100 / int(start.get("cpu_tick_hz", "100")),
        "clock_mhz": statistics.mean(int(r["clock_khz"]) for r in rows) / 1000,
        "temp_c": statistics.mean(int(r["temp_millic"]) for r in rows) / 1000,
        "temp_start_c": int(start["temp_millic"]) / 1000,
        "temp_end_c": int(end["temp_millic"]) / 1000,
        "display_hz": (int(end["lcd_irqs"]) - int(start["lcd_irqs"])) / span,
    }


class Device:
    def __init__(self, args):
        self.args = args
        self.base = ["adb", "-s", args.serial]
        self.output = args.output.resolve()
        self.output.mkdir(parents=True, exist_ok=True)
        self.session = f"/tmp/slot-refresh-{int(time.time())}"
        self.pid = None
        self.original_pid = None
        self.original_stopped = False
        self.process = None
        self.log_file = None
        result_path = self.output / "results.json"
        self.results = json.loads(result_path.read_text()) if args.resume and result_path.is_file() else []
        if result_path.is_file() and not args.resume:
            raise RuntimeError("results already exist; use --resume or a fresh output directory")
        cases = [dict(scene=scene, mode=mode, repeat=repeat, latency=False)
                 for scene in ([] if args.skip_profile else args.scenes)
                 for repeat in range(1, args.repeats + 1) for mode in args.modes
                 if not MODES[mode][1] or (scene.startswith("gba-") and scene.endswith("mgba"))]
        if args.latency_rom:
            cases += [dict(scene="gba-mgba", mode=mode, repeat=repeat, latency=True)
                      for repeat in range(1, args.repeats + 1) for mode in args.modes]
        if args.replan:
            for result in self.results:
                case = {key: result[key] for key in ("scene", "mode", "repeat", "latency")}
                if case not in cases:
                    cases.append(case)
        plan = dict(seconds=args.seconds, warmup=args.warmup, repeats=args.repeats,
                    latency_count=args.latency_count, cases=cases)
        plan_path = self.output / "plan.json"
        if args.resume and not args.replan and plan_path.is_file() and json.loads(plan_path.read_text()) != plan:
            raise RuntimeError("resume would mix different measurement settings")
        plan_path.write_text(json.dumps(plan, indent=2))
        self.original_high = False
        self.mode_touched = False
        self.q = shlex.quote

    def adb(self, *args, timeout=60):
        result = subprocess.run(self.base + list(args), check=True, capture_output=True, timeout=timeout)
        return result.stdout.decode().replace("\r\n", "\n")

    def shell(self, text, timeout=60):
        return self.adb("shell", text, timeout=timeout)

    def write(self, remote, data):
        local = self.output / ".transfer"
        local.write_bytes(data if isinstance(data, bytes) else data.encode())
        self.adb("push", str(local), remote)
        local.unlink()

    def prepare(self):
        panel = self.shell("tr -d '\\000' < /proc/device-tree/soc@03000000/lcd0@01c0c000/lcd_driver_name").strip()
        if panel != "rg34xxsp_v1":
            raise RuntimeError(f"this probe is only for the tested rg34xxsp_v1 panel, got {panel!r}")
        display = self.shell("cat /sys/class/disp/disp/attr/sys")
        rate = re.search(r"lcd output.*?fps:([0-9.]+)", display, re.S)
        if not rate:
            raise RuntimeError("cannot establish the original LCD mode")
        self.original_high = float(rate[1]) > 90
        (self.output / "original.display").write_text(display)
        metadata = {
            "serial": self.args.serial,
            "binary_sha256": hashlib.sha256(self.args.binary.read_bytes()).hexdigest(),
            "probe_sha256": hashlib.sha256(self.args.probe.read_bytes()).hexdigest(),
            "gpsp_sha256": hashlib.sha256(self.args.gpsp.read_bytes()).hexdigest() if self.args.gpsp else
                self.shell(f"sha256sum {self.q(self.args.card + '/System/gpsp_libretro.so')}").split()[0],
            "original_display_hz": float(rate[1]),
            "device": self.shell("uname -a; uptime; cat /sys/devices/system/cpu/online; "
                                 "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor; "
                                 "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq; "
                                 "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq"),
        }
        metadata_path = self.output / "metadata.json"
        if self.args.resume and metadata_path.is_file():
            previous = json.loads(metadata_path.read_text())
            for key in ("serial", "binary_sha256", "probe_sha256"):
                if metadata[key] != previous[key]:
                    raise RuntimeError(f"resume would mix different {key}")
            if metadata["gpsp_sha256"] != previous.get("gpsp_sha256"):
                if any(r["scene"].endswith("gpsp") for r in self.results):
                    raise RuntimeError("resume would mix different gpSP cores; use a fresh output directory")
                previous["gpsp_sha256"] = metadata["gpsp_sha256"]
                metadata_path.write_text(json.dumps(previous, indent=2))
        else:
            metadata_path.write_text(json.dumps(metadata, indent=2))
        for scene in ([] if self.args.skip_profile else self.args.scenes):
            platform, _, filename = SCENES[scene]
            path = self.args.card + "/Games/" + platform + "/" + filename
            if self.shell(f"if [ -f {self.q(path)} ]; then echo yes; fi").strip() != "yes":
                raise RuntimeError(f"missing test ROM: {path}")
        self.original = self.shell(f"cat {self.q(self.args.card + '/Config/slot.state')}")
        (self.output / "original-slot.state").write_text(self.original)
        self.shell(f"mkdir -p {self.q(self.session)}")
        self.adb("push", str(self.args.binary.resolve()), self.session + "/slot")
        self.adb("push", str(self.args.probe.resolve()), self.session + "/probe")
        if self.args.gpsp:
            self.adb("push", str(self.args.gpsp.resolve()), self.session + "/gpsp_libretro.so")
        self.shell(f"chmod +x {self.q(self.session + '/slot')} {self.q(self.session + '/probe')}")
        for name, value in [("menu-down", 1), ("menu-up", 0)]:
            event = struct.pack("<qqHHi", 0, 0, 1, 0x138, value) + struct.pack("<qqHHi", 0, 0, 0, 0, 0)
            self.write(self.session + "/" + name, event)
        self.original_pid = self.shell("pidof slot || true").strip()
        if not self.original_pid:
            # Invoking the executable via ld-linux changes its process name.
            processes = self.shell("ps")
            needle = self.args.card + "/System/slot"
            self.original_pid = next((line.split()[0] for line in processes.splitlines()
                                      if needle in line.split() and "/bin/sh -c" not in line), "")
        if self.original_pid:
            if settings(self.original).get("cart"):
                raise RuntimeError("eject the current cart before running the isolated profile")
            self.shell(f"kill -STOP {int(self.original_pid)}")
            self.original_stopped = True
            time.sleep(0.5)
            proc = self.shell(f"cat /proc/{int(self.original_pid)}/stat")
            if proc.rsplit(")", 1)[1].split()[0] != "T":
                raise RuntimeError("the original Slot process did not stop")
            (self.output / "isolation.txt").write_text(self.shell("ps"))

    def root(self, scene, mode, repeat, latency=False):
        platform, core, filename = SCENES[scene]
        root = self.session + "/card"
        stem = pathlib.PurePath(filename).stem
        q = self.q
        resume = self.args.card + f"/States/{platform}/{core}/{stem}/resume.state"
        resumed = not latency and self.shell(f"if [ -f {q(resume)} ]; then echo yes; fi").strip() == "yes"
        self.shell(f"rm -rf {q(root)}; mkdir -p {q(root + '/Config')} {q(root + '/System')} "
                   f"{q(root + '/Games/' + platform)} {q(root + '/Labels/' + platform)} "
                   f"{q(root + '/Saves/' + platform)} {q(root + '/States/' + platform + '/' + core + '/' + stem)}")
        for library in ("mgba_libretro.so", "gpsp_libretro.so"):
            source = self.session + "/gpsp_libretro.so" if library == "gpsp_libretro.so" and self.args.gpsp else self.args.card + "/System/" + library
            self.shell(f"ln -s {q(source)} {q(root + '/System/' + library)}")
        if not latency:
            self.shell(f"ln -s {q(self.args.card + '/BIOS')} {q(root + '/BIOS')}")
        game = root + "/Games/" + platform + "/" + filename
        if latency:
            self.adb("push", str(self.args.latency_rom.resolve()), game)
        else:
            self.shell(f"ln -s {q(self.args.card + '/Games/' + platform + '/' + filename)} {q(game)}")
            for relative in [f"Saves/{platform}/{stem}.sav", f"States/{platform}/{core}/{stem}/resume.state"]:
                source = self.args.card + "/" + relative
                target = root + "/" + relative
                self.shell(f"if [ -f {q(source)} ]; then cp {q(source)} {q(target)}; fi")
        label = self.args.card + "/Labels/" + platform + "/" + stem + ".png"
        self.shell(f"if [ -f {q(label)} ]; then ln -s {q(label)} {q(root + '/Labels/' + platform + '/' + stem + '.png')}; fi")
        high, ahead = MODES[mode]
        state = settings(self.original)
        state.update(cart=stem, cart_platform=platform.lower(), clock_set="1", high_refresh=str(int(high)),
                     runahead=str(ahead), turbo="0" if latency else state.get("turbo", "1"))
        if latency:
            state.update(shader_gba="off", colour_correction="0", rewind="0")
        self.write(root + "/Config/slot.state", "".join(f"{k}={v}\n" for k, v in state.items()))
        self.write(root + "/Config/selected_core.txt", f"{stem} = {core}\n")
        tag = f"{scene}-{mode}-{repeat}" + ("-latency" if latency else "")
        log = self.output / (tag + ".log")
        self.response_trace = self.session + "/" + tag + "-response.csv"
        pid_file = self.session + "/active.pid"
        self.shell(f"rm -f {q(pid_file)}")
        trace_env = f"SLOT_TRACE_RESPONSE={q(self.response_trace)} " if latency else ""
        command = (f"echo $$ >{q(pid_file)}; exec env SLOT_ROOT={q(root)} SLOT_TRACE=1 SLOT_TRACE_LATENCY=1 {trace_env}"
                   f"/lib/ld-linux-aarch64.so.1 {q(self.session + '/slot')}")
        self.log_file = log.open("wb")
        self.mode_touched = True
        self.process = subprocess.Popen(self.base + ["shell", command], stdout=self.log_file, stderr=subprocess.STDOUT)
        for _ in range(30):
            pid = self.shell(f"cat {q(pid_file)} 2>/dev/null || true").strip()
            if pid:
                self.pid = int(pid)
                break
            time.sleep(0.1)
        else:
            raise RuntimeError("test process did not report its PID")
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            text = log.read_text(errors="replace")
            if self.process.poll() is not None:
                raise RuntimeError(f"test app exited with {self.process.returncode}: {text[-2000:]}")
            if "launch: first frame shown" in text:
                break
            if "slot: load:" in text or "slot: refresh:" in text:
                raise RuntimeError(f"launch failed: {text[-2000:]}")
            time.sleep(0.2)
        else:
            raise RuntimeError(f"no first game frame for {tag}: {text[-2000:]}")
        # Cold boots can cross the existing ALSA silence-release/reopen path near
        # nine seconds. Measure settled emulation rather than that transition.
        time.sleep(max(self.args.warmup, 12 if not latency and not resumed else 0))
        return tag, log, core, platform

    def stop(self, log=None, tag=None):
        if self.pid is None:
            if self.process:
                # PID is written before exec, including failed launches.
                pid = self.shell(f"cat {self.q(self.session + '/active.pid')} 2>/dev/null || true").strip()
                if pid:
                    self.pid = int(pid)
                    return self.stop(log, tag)
                self.process.terminate()
                self.process.wait(timeout=10)
                self.process = None
            if self.log_file:
                self.log_file.close()
                self.log_file = None
            return
        q = self.q
        self.shell(f"cat {q(self.session + '/menu-down')} > /dev/input/event1; sleep 1.1; "
                   f"cat {q(self.session + '/menu-up')} > /dev/input/event1; sleep 0.3")
        self.shell(f"kill -TERM {self.pid} 2>/dev/null || true")
        for _ in range(30):
            alive = self.shell(f"if [ -e /proc/{self.pid}/stat ]; then cat /proc/{self.pid}/stat; fi")
            if not alive or alive.rsplit(")", 1)[1].split()[0] == "Z":
                break
            time.sleep(0.1)
        else:
            raise RuntimeError(f"test process {self.pid} did not exit; panel mode was not changed")
        if self.process:
            self.process.wait(timeout=10)
            self.process = None
        if self.log_file:
            self.log_file.close()
            self.log_file = None
        self.pid = None

    def measure(self, scene, mode, repeat, latency=False):
        try:
            return self._measure(scene, mode, repeat, latency)
        except Exception as error:
            path = self.output / "failures.json"
            failures = json.loads(path.read_text()) if path.is_file() else []
            failures.append(dict(scene=scene, mode=mode, repeat=repeat, latency=latency,
                                 error=str(error)[-2500:]))
            path.write_text(json.dumps(failures, indent=2))
            raise

    def _measure(self, scene, mode, repeat, latency=False):
        if any((r["scene"], r["mode"], r["repeat"], r["latency"]) ==
               (scene, mode, repeat, latency) for r in self.results):
            return
        tag, log, core, platform = self.root(scene, mode, repeat, latency)
        try:
            audio_pattern = r"slot: audio: \d+/\d+ queued, (\d+) dropped, (\d+) starved"
            before_faults = re.findall(audio_pattern, log.read_text(errors="replace"))
            display = self.shell("cat /sys/class/disp/disp/attr/sys")
            (self.output / (tag + ".display")).write_text(display)
            if latency:
                try:
                    text = self.shell(f"{self.q(self.session + '/probe')} response /dev/input/event1 {self.args.latency_count} "
                                      f"{self.q(self.response_trace)} 2>{self.q(self.session + '/response.err')}",
                                      timeout=self.args.latency_count + 30)
                except subprocess.CalledProcessError as error:
                    (self.output / (tag + ".csv")).write_bytes(error.stdout)
                    raise RuntimeError(self.shell(f"cat {self.q(self.session + '/response.err')}")) from error
                self.adb("pull", self.response_trace, str(self.output / (tag + "-response.csv")))
                (self.output / (tag + ".csv")).write_text(text)
                rows = list(csv.DictReader(io.StringIO(text)))
                values = [float(row["latency_ms"]) for row in rows]
                result = dict(latency_median_ms=statistics.median(values), latency_mean_ms=statistics.mean(values),
                              latency_min_ms=min(values), latency_max_ms=max(values), trials=len(values))
            else:
                kind = "gpsp" if core == "gpsp" else "gba" if platform == "GBA" else "gb"
                text = self.shell(f"{self.q(self.session + '/probe')} sample {self.pid} {kind} {self.args.seconds}",
                                  timeout=self.args.seconds + 30)
                (self.output / (tag + ".csv")).write_text(text)
                result = summarize(text)
                target_hz = 120 if MODES[mode][0] else 60
                if abs(result["display_hz"] - target_hz) > 3:
                    raise RuntimeError(f"{tag}: requested {target_hz} Hz but measured {result['display_hz']:.2f} Hz")
            result.update(scene=scene, mode=mode, repeat=repeat, latency=latency)
            diagnostics = log.read_text(errors="replace")
            faults = re.findall(audio_pattern, diagnostics)
            if before_faults and faults:
                result.update(audio_drop_delta=int(faults[-1][0]) - int(before_faults[-1][0]),
                              audio_starve_delta=int(faults[-1][1]) - int(before_faults[-1][1]))
            nominal = re.search(r"slot: emu: nominal ([0-9.]+) fps", diagnostics)
            if nominal and not latency:
                result["speed_percent"] = result["game_fps"] / float(nominal[1]) * 100
            timings = re.findall(r"([0-9.]+) ms core/present", diagnostics)
            if timings:
                result["core_ms"] = float(timings[-1])
            if "slot: refresh:" in diagnostics or "missed game periods" in diagnostics and re.search(
                    r"[1-9][0-9]* missed game periods", diagnostics):
                result["timing_warning"] = True
            self.results.append(result)
            (self.output / "results.json").write_text(json.dumps(self.results, indent=2))
            print(json.dumps(result), flush=True)
        finally:
            self.stop(log, tag)

    def restore(self):
        self.stop()
        if self.mode_touched:
            # Restore the user's initially selected physical mode before reopening the menu.
            if self.original_high:
                self.shell("echo 0x06511044 0xf000000a > /sys/class/sunxi_dump/write; "
                           "echo 0x03001040 0x88002b03 > /sys/class/sunxi_dump/write")
            else:
                self.shell("echo 0x03001040 0x88001703 > /sys/class/sunxi_dump/write; "
                           "echo 0x06511044 0xf000000b > /sys/class/sunxi_dump/write")
        if self.original_stopped:
            # Its evdev queues contain our injected inputs. Restart the idle
            # original through init instead of replaying those keys on resume.
            self.shell(f"kill -KILL {int(self.original_pid)}")
            self.original_stopped = False


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--serial", required=True)
    p.add_argument("--binary", type=pathlib.Path, required=True)
    p.add_argument("--probe", type=pathlib.Path, required=True)
    p.add_argument("--gpsp", type=pathlib.Path, help="test an isolated gpSP build without installing it")
    p.add_argument("--output", type=pathlib.Path, required=True)
    p.add_argument("--card", default="/mnt/sdcard")
    p.add_argument("--seconds", type=float, default=20)
    p.add_argument("--warmup", type=float, default=5)
    p.add_argument("--repeats", type=int, default=5)
    p.add_argument("--scenes", nargs="+", choices=SCENES, default=list(SCENES))
    p.add_argument("--modes", nargs="+", choices=MODES, default=["60", "120", "runahead"])
    p.add_argument("--latency-rom", type=pathlib.Path)
    p.add_argument("--latency-count", type=int, default=60)
    p.add_argument("--skip-profile", action="store_true")
    p.add_argument("--resume", action="store_true", help="keep completed results with identical binaries")
    p.add_argument("--replan", action="store_true", help="change remaining cases while retaining completed samples")
    a = p.parse_args()
    d = Device(a)
    try:
        d.prepare()
        for scene in ([] if a.skip_profile else a.scenes):
            for repeat in range(a.repeats):
                modes = list(a.modes)
                modes = modes[repeat % len(modes):] + modes[:repeat % len(modes)]
                for mode in modes:
                    if MODES[mode][1] and not (scene.startswith("gba-") and scene.endswith("mgba")):
                        continue
                    d.measure(scene, mode, repeat + 1)
        if a.latency_rom:
            for repeat in range(a.repeats):
                modes = list(a.modes)
                modes = modes[repeat % len(modes):] + modes[:repeat % len(modes)]
                for mode in modes:
                    d.measure("gba-mgba", mode, repeat + 1, latency=True)
    finally:
        d.restore()


if __name__ == "__main__":
    main()
