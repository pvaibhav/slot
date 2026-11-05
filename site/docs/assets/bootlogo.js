(() => {
  const DEVICES = [
    { id: "rg35xxplus", name: "RG35XX Plus / RG35XX 2024", width: 640, height: 480, rotation: 0 },
    { id: "rg35xxh", name: "RG35XX H", width: 640, height: 480, rotation: 0 },
    { id: "rg35xxpro", name: "RG35XX Pro", width: 640, height: 480, rotation: 0 },
    { id: "rg35xxsp", name: "RG35XX SP", width: 640, height: 480, rotation: 0 },
    { id: "rg40xxh", name: "RG40XX H", width: 640, height: 480, rotation: 0 },
    { id: "rg40xxv", name: "RG40XX V", width: 640, height: 480, rotation: 0 },
    { id: "rgsp", name: "RG SP", width: 720, height: 480, rotation: 0 },
    { id: "rg34xx", name: "RG34XX", width: 720, height: 480, rotation: 0 },
    { id: "rg34xxsp", name: "RG34XX SP", width: 720, height: 480, rotation: 0 },
    { id: "rgcubexx", name: "RG CubeXX", width: 720, height: 720, rotation: 0 },
    { id: "rg28xx", name: "RG28XX", width: 640, height: 480, rotation: 90 },
  ];
  const FALLBACK = "rgsp";
  const SMALLEST = 8;

  const $ = (id) => document.getElementById(id);
  const select = $("blm-device");
  const fileInput = $("blm-file");
  const pick = $("blm-pick");
  const chip = $("blm-chip");
  const thumb = $("blm-thumb");
  const nameTag = $("blm-name");
  const sizeTag = $("blm-size");
  const error = $("blm-error");
  const background = $("blm-background");
  const frame = $("blm-frame");
  const preview = $("blm-preview");
  const box = $("blm-box");
  const ghost = $("blm-ghost");
  const stage = frame.parentElement;
  const keep = $("blm-keep");
  const download = $("blm-download");
  const modes = [...document.querySelectorAll('input[name="blm-mode"]')];
  if (!select || !preview) return;

  let image = null;
  let imageUrl = null;
  let rect = null;
  let preset = "fit";

  const groups = new Map();
  for (const d of DEVICES) {
    const size = d.rotation ? `${d.height}×${d.width}` : `${d.width}×${d.height}`;
    if (!groups.has(size)) {
      const group = document.createElement("optgroup");
      group.label = size;
      groups.set(size, group);
      select.append(group);
    }
    const option = document.createElement("option");
    option.value = d.id;
    option.textContent = d.name;
    groups.get(size).append(option);
  }

  let saved = null;
  try {
    saved = localStorage.getItem("bootlogo-device");
  } catch {}
  select.value = DEVICES.some((d) => d.id === saved) ? saved : FALLBACK;

  function device() {
    return DEVICES.find((d) => d.id === select.value);
  }

  function placed(d, m) {
    const iw = image.naturalWidth;
    const ih = image.naturalHeight;
    if (m === "center") return { x: (d.width - iw) / 2, y: (d.height - ih) / 2, w: iw, h: ih };
    const scale = m === "fill"
      ? Math.max(d.width / iw, d.height / ih)
      : Math.min(d.width / iw, d.height / ih);
    const w = iw * scale;
    const h = ih * scale;
    return { x: (d.width - w) / 2, y: (d.height - h) / 2, w, h };
  }

  function applyPreset(m) {
    preset = m;
    for (const radio of modes) radio.checked = radio.value === m;
    if (image) rect = placed(device(), m);
    render();
  }

  function manual() {
    for (const radio of modes) radio.checked = false;
  }

  function upright(d) {
    const canvas = document.createElement("canvas");
    canvas.width = d.width;
    canvas.height = d.height;
    const ctx = canvas.getContext("2d");
    ctx.fillStyle = background.value;
    ctx.fillRect(0, 0, d.width, d.height);
    if (image && rect) {
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(image, rect.x, rect.y, rect.w, rect.h);
    }
    return canvas;
  }

  function turned(source, rotation) {
    if (rotation !== 90) return source;
    const canvas = document.createElement("canvas");
    canvas.width = source.height;
    canvas.height = source.width;
    const ctx = canvas.getContext("2d");
    ctx.setTransform(0, -1, 1, 0, 0, source.width);
    ctx.drawImage(source, 0, 0);
    return canvas;
  }

  function bmp(canvas) {
    const { width, height } = canvas;
    const rgba = canvas.getContext("2d").getImageData(0, 0, width, height).data;
    const stride = (width * 3 + 3) & ~3;
    const size = 54 + stride * height;
    const out = new Uint8Array(size);
    const view = new DataView(out.buffer);
    out[0] = 0x42;
    out[1] = 0x4d;
    view.setUint32(2, size, true);
    view.setUint32(10, 54, true);
    view.setUint32(14, 40, true);
    view.setInt32(18, width, true);
    view.setInt32(22, height, true);
    view.setUint16(26, 1, true);
    view.setUint16(28, 24, true);
    view.setUint32(30, 0, true);
    view.setUint32(34, stride * height, true);
    view.setInt32(38, 2835, true);
    view.setInt32(42, 2835, true);
    for (let y = 0; y < height; y++) {
      const row = 54 + (height - 1 - y) * stride;
      for (let x = 0; x < width; x++) {
        const from = (y * width + x) * 4;
        const to = row + x * 3;
        out[to] = rgba[from + 2];
        out[to + 1] = rgba[from + 1];
        out[to + 2] = rgba[from];
      }
    }
    return new Blob([out], { type: "image/bmp" });
  }

  function render() {
    const d = device();
    preview.width = d.width;
    preview.height = d.height;
    frame.style.setProperty("--ar", `${d.width} / ${d.height}`);
    frame.style.setProperty("--ratio", d.width / d.height);
    preview.getContext("2d").drawImage(upright(d), 0, 0);
    download.disabled = !image;
    box.hidden = ghost.hidden = !image || !rect;
    if (rect) {
      for (const el of [box, ghost]) {
        el.style.left = `${(rect.x / d.width) * 100}%`;
        el.style.top = `${(rect.y / d.height) * 100}%`;
        el.style.width = `${(rect.w / d.width) * 100}%`;
        el.style.height = `${(rect.h / d.height) * 100}%`;
      }
      pin(d);
    }
  }

  function pin(d) {
    const f = frame.getBoundingClientRect();
    const s = stage.getBoundingClientRect();
    if (!f.width) return;
    const k = f.width / d.width;
    const inset = 9;
    const bx = rect.x * k;
    const by = rect.y * k;
    const clamp = (v, lo, hi) => Math.min(Math.max(v, lo), hi);
    for (const handle of box.querySelectorAll(".blm-handle")) {
      const c = handle.dataset.corner;
      const cx = (c.includes("w") ? rect.x : rect.x + rect.w) * k;
      const cy = (c.includes("n") ? rect.y : rect.y + rect.h) * k;
      const x = clamp(cx, s.left - f.left + inset, s.right - f.left - inset);
      const y = clamp(cy, s.top - f.top + inset, s.bottom - f.top - inset);
      handle.style.left = `${x - bx}px`;
      handle.style.top = `${y - by}px`;
      handle.classList.toggle("is-pinned", x !== cx || y !== cy);
    }
  }

  function fail(message) {
    error.textContent = message;
    error.hidden = false;
  }

  function load(file) {
    if (!file) return;
    error.hidden = true;
    const url = URL.createObjectURL(file);
    const next = new Image();
    next.onload = () => {
      if (imageUrl) URL.revokeObjectURL(imageUrl);
      image = next;
      imageUrl = url;
      thumb.src = url;
      ghost.style.backgroundImage = `url("${url}")`;
      const name = file.name || "Pasted image";
      nameTag.textContent = name;
      nameTag.title = name;
      sizeTag.textContent = `${next.naturalWidth}×${next.naturalHeight}`;
      pick.hidden = true;
      chip.hidden = false;
      applyPreset(preset);
    };
    next.onerror = () => {
      URL.revokeObjectURL(url);
      fail("Your browser could not open that file. Try a PNG or JPG.");
    };
    next.src = url;
  }

  let drag = null;

  function startDrag(e, corner) {
    if (!rect || e.button > 0) return;
    e.preventDefault();
    e.stopPropagation();
    const d = device();
    drag = {
      corner,
      id: e.pointerId,
      sx: e.clientX,
      sy: e.clientY,
      scale: d.width / frame.getBoundingClientRect().width,
      from: { ...rect },
    };
    box.setPointerCapture(e.pointerId);
    box.classList.add("is-dragging");
  }

  function moveDrag(e) {
    if (!drag || e.pointerId !== drag.id) return;
    const dx = (e.clientX - drag.sx) * drag.scale;
    const dy = (e.clientY - drag.sy) * drag.scale;
    const f = drag.from;
    if (!drag.corner) {
      rect = { ...f, x: f.x + dx, y: f.y + dy };
    } else {
      const west = drag.corner.includes("w");
      const north = drag.corner.includes("n");
      const ax = west ? f.x + f.w : f.x;
      const ay = north ? f.y + f.h : f.y;
      let w = Math.max(SMALLEST, (west ? -1 : 1) * ((west ? f.x : f.x + f.w) + dx - ax));
      let h = Math.max(SMALLEST, (north ? -1 : 1) * ((north ? f.y : f.y + f.h) + dy - ay));
      if (keep.checked) {
        const ratio = image.naturalWidth / image.naturalHeight;
        const rw = w / f.w;
        const rh = h / f.h;
        w = Math.max(SMALLEST, f.w * (Math.abs(rw - 1) > Math.abs(rh - 1) ? rw : rh));
        h = w / ratio;
      }
      rect = { x: west ? ax - w : ax, y: north ? ay - h : ay, w, h };
    }
    manual();
    render();
  }

  function endDrag(e) {
    if (!drag || e.pointerId !== drag.id) return;
    drag = null;
    box.classList.remove("is-dragging");
  }

  box.addEventListener("pointerdown", (e) => startDrag(e, null));
  for (const handle of box.querySelectorAll(".blm-handle")) {
    handle.addEventListener("pointerdown", (e) => startDrag(e, handle.dataset.corner));
  }
  box.addEventListener("pointermove", moveDrag);
  box.addEventListener("pointerup", endDrag);
  box.addEventListener("pointercancel", endDrag);

  keep.addEventListener("change", () => {
    if (!keep.checked || !rect || !image) return;
    const ratio = image.naturalWidth / image.naturalHeight;
    const cy = rect.y + rect.h / 2;
    const h = rect.w / ratio;
    rect = { ...rect, y: cy - h / 2, h };
    manual();
    render();
  });

  select.addEventListener("change", () => {
    try {
      localStorage.setItem("bootlogo-device", select.value);
    } catch {}
    applyPreset(preset);
  });
  background.addEventListener("input", render);
  for (const radio of modes) {
    radio.addEventListener("change", () => applyPreset(radio.value));
    radio.addEventListener("click", () => {
      if (radio.value === preset && radio.checked) applyPreset(radio.value);
    });
  }
  fileInput.addEventListener("change", () => {
    load(fileInput.files[0]);
    fileInput.value = "";
  });
  for (const label of document.querySelectorAll('label[for="blm-file"]')) {
    label.addEventListener("keydown", (e) => {
      if (e.key !== "Enter" && e.key !== " ") return;
      e.preventDefault();
      fileInput.click();
    });
  }
  document.addEventListener("paste", (e) => {
    const item = [...(e.clipboardData?.items || [])].find((i) => i.kind === "file");
    if (item) load(item.getAsFile());
  });
  download.addEventListener("click", () => {
    if (!image) return;
    const d = device();
    const blob = bmp(turned(upright(d), d.rotation));
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = "bootlogo.bmp";
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(link.href), 1000);
  });

  window.addEventListener("resize", render);
  render();
})();
