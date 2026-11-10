(function(){
  "use strict";

  fetch(new URL("device.svg", document.currentScript.src))
    .then(function(r){ return r.text(); })
    .then(function(txt){
      var holder = document.createElement("div");
      holder.innerHTML = txt;
      var defs = holder.querySelector("svg");
      if (!defs) return;
      document.body.insertBefore(defs, document.body.firstChild);
      var peerTxt = txt
        .replace(/id="([\w-]+)"/g, 'id="$1-peer"')
        .replace(/url\(#([\w-]+)\)/g, "url(#$1-peer)")
        .replace(/href="#([\w-]+)"/g, 'href="#$1-peer"');
      holder.innerHTML = peerTxt;
      var peerDefs = holder.querySelector("svg");
      if (peerDefs) {
        peerDefs.classList.add("peer-defs");
        document.body.insertBefore(peerDefs, defs.nextSibling);
      }
      var uses = document.querySelectorAll("use");
      for (var i = 0; i < uses.length; i++) {
        var h = uses[i].getAttribute("href");
        uses[i].removeAttribute("href");
        uses[i].setAttribute("href", h);
      }
    })
    .catch(function(){});
})();

(function(){
  "use strict";

  var MEDIA = new URL("../media/", document.currentScript.src).href;

  var SHELLS = [
    { name:"silver",     light:"#e2e5e9", base:"#b8bcc2", shade:"#7e838b", btn:"#c2c6cc", btnHi:"#e9ecf0", etch:"rgba(0,0,0,.46)" },
    { name:"pink",       light:"#f6d3dd", base:"#e6a6b8", shade:"#a76d7e", btn:"#dbd1d5", btnHi:"#f2eaed", etch:"rgba(0,0,0,.44)" },
    { name:"light blue", light:"#dbe9f2", base:"#a8c4d8", shade:"#6d8ba1", btn:"#d0dae3", btnHi:"#edf3f8", etch:"rgba(0,0,0,.44)" },
    { name:"black",      light:"#5a5d64", base:"#34363b", shade:"#16171a", btn:"#3c3e43", btnHi:"#5c5f66", etch:"rgba(255,255,255,.42)" }
  ];

  var want = (location.search.match(/[?&]shell=([^&]+)/) || [])[1];
  var shell = SHELLS[Math.floor(Math.random() * SHELLS.length)];
  if (want) {
    want = decodeURIComponent(want).replace(/\+/g, " ").toLowerCase();
    for (var si = 0; si < SHELLS.length; si++) {
      if (SHELLS[si].name === want) { shell = SHELLS[si]; break; }
    }
  }

  var root = document.documentElement.style;
  root.setProperty("--shell-light", shell.light);
  root.setProperty("--shell-base",  shell.base);
  root.setProperty("--shell-shade", shell.shade);
  root.setProperty("--btn",         shell.btn);
  root.setProperty("--btn-hi",      shell.btnHi);
  root.setProperty("--etch",        shell.etch);

  var pose = (location.search.match(/[?&]pose=(flat|hero)/) || [])[1];
  if (pose) {
    var heroDevEl = document.querySelector(".hero-device .device");
    if (heroDevEl) heroDevEl.className = "device pose-" + pose;
  }


  var MOVED = {
    install: "installing/", "install-single": "installing/",
    "install-dual": "installing/#two-sd-cards", "install-update": "updating/",
    guide: "tour/", buttons: "guide/controls/#defaults", controls: "guide/controls/#everywhere",
    "controls-carousel": "guide/controls/#carousel", "controls-in-game": "guide/controls/#in-game",
    "controls-switcher": "guide/controls/#save-state-switcher",
    questions: "questions/", faq: "questions/", "faq-games": "guide/storage/",
    "faq-shelf": "guide/customizing/", "faq-linking": "guide/linking/",
    "faq-requests": "questions/",
    credits: "credits/", disclosure: "ai/"
  };
  ["browse", "favorites", "core-picker", "insert", "eject", "save-states", "switcher", "fast-forward",
   "expand", "shaders", "palettes", "link", "settings", "brightness", "lid-detection"].forEach(function(id){
    MOVED[id] = "tour/#" + id;
  });
  var moved = document.querySelector(".hero") && MOVED[location.hash.slice(1)];
  if (moved) { location.replace(new URL("../" + moved, document.currentScript.src).href); return; }

  function openWay(){
    var way = location.hash && document.getElementById(decodeURIComponent(location.hash.slice(1)));
    if (!way) return;
    for (var d = way.closest("details"); d; d = d.parentElement && d.parentElement.closest("details")) d.open = true;
    way.scrollIntoView();
  }
  window.addEventListener("hashchange", openWay);
  openWay();

  var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  var heroVid = document.querySelector(".hero-screen");
  if (heroVid) {
    if (reduced) {
      heroVid.style.opacity = "1";
    } else {
      setTimeout(function(){
        heroVid.style.opacity = "1";
        var p = heroVid.play();
        if (p && p.catch) p.catch(function(){});
      }, 1900);
      heroVid.src = MEDIA + "main.mp4";
      heroVid.load();
    }
  }


  var source = document.querySelector(".guide-steps");
  if (source) {
    var heads = Array.prototype.slice.call(source.querySelectorAll("h3[id]"));
    heads.forEach(function(h){
      var step = document.createElement("div");
      step.className = "step";
      step.id = h.id;
      h.removeAttribute("id");
      ["data-clip", "data-still", "data-pair", "data-lid"].forEach(function(a){
        if (h.hasAttribute(a)) { step.setAttribute(a, h.getAttribute(a)); h.removeAttribute(a); }
      });
      var link = h.querySelector(".headerlink");
      if (link) h.removeChild(link);
      h.parentNode.insertBefore(step, h);
      for (var node = h; node && !(node.tagName === "H3" && node !== h);) {
        var next = node.nextSibling;
        step.appendChild(node);
        node = next;
      }
    });
  }

  var vid   = document.querySelector(".guide-screen");
  var gdev  = document.querySelector(".guide-stage .device");
  var steps = Array.prototype.slice.call(document.querySelectorAll(".step"));

  var stage = gdev ? gdev.parentNode : null;
  var peer = null, pvid = null;
  if (gdev) {
    peer = gdev.cloneNode(true);
    peer.classList.add("is-peer");
    peer.classList.remove("is-waiting", "is-shut", "is-swinging");
    peer.setAttribute("aria-hidden", "true");
    var others = SHELLS.filter(function(x){ return x !== shell; });
    var their = others[Math.floor(Math.random() * others.length)];
    var peerVars = { "light":their.light, "base":their.base, "shade":their.shade,
                     "btn":their.btn, "btn-hi":their.btnHi };
    for (var pk in peerVars) root.setProperty("--peer-" + pk, peerVars[pk]);
    peer.style.setProperty("--shell-light", their.light);
    peer.style.setProperty("--shell-base",  their.base);
    peer.style.setProperty("--shell-shade", their.shade);
    peer.style.setProperty("--btn",         their.btn);
    peer.style.setProperty("--btn-hi",      their.btnHi);
    peer.style.setProperty("--etch",        their.etch);
    var peerUses = peer.querySelectorAll("use");
    for (var pu = 0; pu < peerUses.length; pu++) {
      peerUses[pu].setAttribute("href", peerUses[pu].getAttribute("href") + "-peer");
    }
    pvid = peer.querySelector("video");
    pvid.classList.remove("guide-screen");
    stage.appendChild(peer);
  }

  var pairOn = false, pairStarting = false;
  function play(v){ var p = v.play(); if (p && p.catch) p.catch(function(){}); }
  function pairReady(){ return vid.readyState >= 3 && pvid.readyState >= 3; }
  function startPair(){
    if (!pairOn || pairStarting || !pairReady()) return;
    pairStarting = true;
    vid.pause(); pvid.pause();
    pvid.playbackRate = 1;
    var left = 2, done = false;
    function go(){
      if (done || --left > 0) return;
      done = true;
      pairStarting = false;
      if (!pairOn) return;
      play(vid); play(pvid);
    }
    vid.addEventListener("seeked", go, { once:true });
    pvid.addEventListener("seeked", go, { once:true });
    vid.currentTime = 0; pvid.currentTime = 0;
    setTimeout(function(){ left = 1; go(); }, 600);
  }
  if (vid && pvid) {
    ["canplay", "canplaythrough"].forEach(function(e){
      [vid, pvid].forEach(function(v){
        v.addEventListener(e, function(){ if (vid.paused || pvid.paused) startPair(); });
      });
    });
    vid.addEventListener("waiting", function(){ if (pairOn) pvid.pause(); });
    pvid.addEventListener("waiting", function(){ if (pairOn) vid.pause(); });
    vid.addEventListener("ended", function(){ if (pairOn) startPair(); });
    vid.addEventListener("timeupdate", function(){
      if (!pairOn || pairStarting || vid.paused) return;
      if (pvid.paused) { play(pvid); return; }
      var d = vid.currentTime - pvid.currentTime;
      if (Math.abs(d) > 0.3) { startPair(); return; }
      pvid.playbackRate = 1 + Math.max(-0.1, Math.min(0.1, d * 0.5));
    });
  }
  function showPeer(clip){
    if (!stage || !pvid) return;
    stage.classList.toggle("is-pair", !!clip);
    pairOn = !!clip && !reduced;
    vid.loop = !pairOn;
    if (!clip) { pvid.pause(); pvid.removeAttribute("src"); pvid.playbackRate = 1; return; }
    pvid.poster = MEDIA + clip.replace(/\.mp4$/, ".webp");
    if (reduced) { pvid.removeAttribute("src"); return; }
    if (pvid.getAttribute("src") !== MEDIA + clip) {
      pvid.src = MEDIA + clip;
      pvid.load();
    }
  }

  var current = null;

  var LID_SWING_MS = 1150;
  var swingTimer = null;

  var playRetries = [];
  function ensurePlaying(){
    if (reduced || !vid || !vid.getAttribute("src") || !vid.paused) return;
    if (pairOn) { startPair(); return; }
    var p = vid.play();
    if (p && p.catch) p.catch(function(){});
  }
  function nudgePlayback(){
    for (var i = 0; i < playRetries.length; i++) clearTimeout(playRetries[i]);
    playRetries = [];
    ensurePlaying();
    playRetries.push(setTimeout(ensurePlaying, 220), setTimeout(ensurePlaying, 800));
  }
  if (vid) {
    vid.loop = !pairOn;
    vid.addEventListener("loadeddata", ensurePlaying);
    vid.addEventListener("canplay", ensurePlaying);
    vid.addEventListener("canplaythrough", ensurePlaying);
  }

  function show(step){
    if (!vid || !step || step === current) return;
    current = step;
    var at = 0;
    for (var i = 0; i < steps.length; i++) {
      var on = steps[i] === step;
      steps[i].classList.toggle("is-active", on);
      if (on) at = i;
    }
    if (stepAt) stepAt.textContent = String(at + 1);
    if (prevBtn) prevBtn.disabled = at === 0;
    if (nextBtn) nextBtn.disabled = at === steps.length - 1;
    if (gdev) {
      var wantShut = step.getAttribute("data-lid") === "shut";
      if (!wantShut && gdev.classList.contains("is-shut") && !reduced) {
        gdev.classList.add("is-swinging");
        clearTimeout(swingTimer);
        swingTimer = setTimeout(function(){ gdev.classList.remove("is-swinging"); }, LID_SWING_MS);
      }
      gdev.classList.toggle("is-shut", wantShut);
    }

    showPeer(step.getAttribute("data-pair"));
    var only = step.getAttribute("data-still");
    if (only) {
      vid.removeAttribute("src");
      vid.load();
      vid.poster = MEDIA + only;
      vid.style.opacity = "1";
      return;
    }
    var clip = step.getAttribute("data-clip");
    if (!clip) { vid.style.opacity = "0"; vid.removeAttribute("src"); return; }
    var still = MEDIA + clip.replace(/\.mp4$/, ".webp");
    if (reduced) {
      if (vid.poster !== still) { vid.removeAttribute("src"); vid.poster = still; }
      vid.style.opacity = "1";
      return;
    }
    var path = MEDIA + clip;
    if (vid.getAttribute("src") === path) {
      vid.style.opacity = "1";
      nudgePlayback();
      return;
    }
    vid.poster = still;
    vid.src = path;
    vid.style.opacity = "1";
    vid.load();
    nudgePlayback();
  }

  var stepList = document.querySelector(".guide-steps");
  var prevBtn  = document.getElementById("step-prev");
  var nextBtn  = document.getElementById("step-next");
  var stepAt   = document.getElementById("step-at");
  var stepOf   = document.getElementById("step-of");
  if (stepList && steps.length) stepList.classList.add("is-live");
  if (stepOf) stepOf.textContent = String(steps.length);

  function stepBy(delta){
    var at = steps.indexOf(current);
    var to = at + delta;
    if (at < 0 || to < 0 || to >= steps.length) return;
    show(steps[to]);
    setHash(steps[to].id);
  }
  if (prevBtn) prevBtn.addEventListener("click", function(){ stepBy(-1); });
  if (nextBtn) nextBtn.addEventListener("click", function(){ stepBy(1); });

  var guidePanel = document.querySelector(".guide");
  if (guidePanel) guidePanel.addEventListener("keydown", function(e){
    if (e.target.closest("[role=tablist]")) return;
    if (e.key === "ArrowLeft") { e.preventDefault(); stepBy(-1); }
    else if (e.key === "ArrowRight") { e.preventDefault(); stepBy(1); }
  });


  function setHash(id){
    if (id && history.replaceState) history.replaceState(null, "", "#" + id);
  }

  function showHash(){
    var el = document.getElementById(location.hash.slice(1));
    if (el && el.classList.contains("step")) show(el);
  }
  window.addEventListener("hashchange", showHash);

  if (steps.length) show(steps[0]);
  showHash();

  var say = document.querySelector(".say");
  var voice = document.getElementById("say-slot");
  if (say && voice) {
    say.addEventListener("click", function(){
      voice.currentTime = 0;
      var playing = voice.play();
      if (playing && playing.catch) playing.catch(function(){});
    });
    voice.addEventListener("play", function(){ say.classList.add("is-playing"); });
    ["pause", "ended"].forEach(function(e){
      voice.addEventListener(e, function(){ say.classList.remove("is-playing"); });
    });
  }

})();

(function(){
  "use strict";

  var cart = document.querySelector(".doc-cart");
  if (!cart) return;

  var root    = document.documentElement;
  var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var screens = [".md-header", ".md-container"].map(function(s){ return document.querySelector(s); }).filter(Boolean);
  var img     = cart.querySelector("img");

  var BAR = 58, SEAT_GAP = 0, SEATED_W = 240, SHELF_W = 360;
  var CATCH_IN = 0.42, CATCH_OUT = 0.62, CREEP = 0.03;
  var TRAVEL_MS = 450, INSERT_HOLD_MS = 280, EJECT_HOLD_MS = 350, POWER_ON_MS = 220, POWER_OFF_MS = 160, FOOTER_SLIDE_MS = POWER_ON_MS;
  var LINE_PX = 2, DOT_T = 0.12, OVERSHOOT = 0.6, SCROLL_MS = 300;

  var stage = null, free = null, barTop = 0, busy = false, rects = [];

  function ease(u){ return u * u * u * (u * (u * 6 - 15) + 10); }

  function travel(seat, c){
    if (seat < CATCH_IN) return c * ease(seat / CATCH_IN);
    if (seat < CATCH_OUT) return c + CREEP * (seat - CATCH_IN) / (CATCH_OUT - CATCH_IN);
    var caught = c + CREEP;
    return caught + (1 - caught) * ease((seat - CATCH_OUT) / (1 - CATCH_OUT));
  }

  function tall(w){ return w * img.naturalHeight / img.naturalWidth; }

  function cartAt(seat){
    var w0 = Math.min(SHELF_W, innerWidth * 0.8), h0 = tall(w0);
    var sh = tall(SEATED_W);
    var foot0 = Math.min((barTop + BAR) / 2 + h0 / 2, barTop - 8);
    var seated = barTop + SEAT_GAP + sh;
    var c = (barTop - foot0) / (seated - foot0);
    var t = travel(seat, c);
    var k = Math.min(t / c, 1);
    var w = w0 + (SEATED_W - w0) * k;
    var foot = foot0 + (seated - foot0) * t;
    return { left:innerWidth / 2 - w / 2, top:foot - tall(w), width:w };
  }

  function place(seat){
    var p = cartAt(seat);
    free.style.left = p.left + "px";
    free.style.top = p.top + "px";
    free.style.width = p.width + "px";
    var under = p.top + tall(p.width) - (barTop + BAR);
    free.style.clipPath = under > 0 ? "inset(0 0 " + under + "px 0)" : "";
  }

  function run(ms, step){
    return new Promise(function(done){
      if (reduced || ms <= 0) { step(1); done(); return; }
      var t0 = performance.now();
      (function frame(now){
        var p = Math.min((now - t0) / ms, 1);
        step(p);
        if (p < 1) requestAnimationFrame(frame); else done();
      })(t0);
    });
  }

  function measure(){
    unpower();
    rects = screens.map(function(el){
      var r = el.getBoundingClientRect();
      return { top:r.top, height:r.height };
    });
  }

  function power(t){
    var view = Math.max(barTop, LINE_PX);
    var left = 1 - t;
    var sy = 1 - (1 - LINE_PX / view) * left * left;
    var sx = 1 - (1 - LINE_PX / innerWidth) * (1 - Math.min(t / DOT_T, 1));
    var bright = 1 + OVERSHOOT * left * left;
    screens.forEach(function(el, i){
      var r = rects[i];
      el.style.clipPath = "inset(" + Math.max(0, -r.top) + "px 0 " + Math.max(0, r.top + r.height - barTop) + "px 0)";
      el.style.transformOrigin = "50% " + (barTop / 2 - r.top) + "px";
      el.style.transform = "scale(" + sx + "," + sy + ")";
      el.style.filter = "brightness(" + bright + ")";
    });
  }

  function unpower(){
    screens.forEach(function(el){
      el.style.clipPath = el.style.transformOrigin = el.style.transform = el.style.filter = "";
    });
  }

  function scrollBarDown(){
    var bar = cart.parentNode.getBoundingClientRect().top;
    var max = root.scrollHeight - innerHeight;
    var from = scrollY;
    var to = Math.max(0, Math.min(max, from + bar + BAR - innerHeight));
    if (Math.abs(to - from) < 1) return Promise.resolve();
    return run(SCROLL_MS, function(p){ window.scrollTo({ top:from + (to - from) * ease(p), behavior:"instant" }); });
  }

  function hold(since, ms){
    var left = ms - (performance.now() - since);
    return new Promise(function(done){ setTimeout(done, reduced ? 0 : Math.max(0, left)); });
  }

  function build(){
    stage = document.createElement("div");
    stage.className = "doc-stage";
    var back = document.createElement("div");
    back.className = "doc-stage-bar";
    back.style.top = barTop + "px";
    var front = document.createElement("div");
    front.className = "doc-stage-front";
    front.style.top = barTop + "px";
    free = cart.cloneNode(true);
    free.classList.add("is-free");
    free.setAttribute("aria-label", "Insert the docs");
    free.addEventListener("click", insert);
    stage.appendChild(back);
    stage.appendChild(free);
    stage.appendChild(front);
    document.body.appendChild(stage);
  }

  function eject(){
    if (busy || stage) return;
    busy = true;
    scrollBarDown().then(function(){
      barTop = cart.parentNode.getBoundingClientRect().top;
      build();
      place(1);
      cart.style.visibility = "hidden";
      root.classList.add("docs-dark");
      measure();
      var since = performance.now();
      return run(POWER_OFF_MS, function(p){ power(1 - p); }).then(function(){
        root.classList.add("docs-off");
        screens.forEach(function(el){ el.inert = true; });
        return hold(since, EJECT_HOLD_MS);
      });
    }).then(function(){
      return run(TRAVEL_MS, function(p){ place(1 - p); });
    }).then(function(){
      free.classList.add("is-parked");
      free.focus({ preventScroll:true });
      busy = false;
    });
  }

  function insert(){
    if (busy || !stage) return;
    busy = true;
    free.classList.remove("is-parked");
    screens.forEach(function(el){ el.style.opacity = "0"; });
    root.classList.remove("docs-off");
    window.scrollTo({ top:0, behavior:"instant" });
    run(TRAVEL_MS, function(p){ place(p); }).then(function(){
      return hold(performance.now(), INSERT_HOLD_MS);
    }).then(function(){
      measure();
      power(0);
      screens.forEach(function(el){ el.style.opacity = ""; el.inert = false; });
      return run(POWER_ON_MS, power);
    }).then(function(){
      unpower();
      if (Math.abs(cart.parentNode.getBoundingClientRect().top - barTop) < 1) {
        cart.style.visibility = "";
        return run(POWER_OFF_MS, function(p){ stage.style.opacity = String(1 - p); });
      }
      var drop = innerHeight - barTop;
      return run(FOOTER_SLIDE_MS, function(p){ stage.style.transform = "translateY(" + drop * p + "px)"; });
    }).then(function(){
      stage.remove();
      stage = free = null;
      root.classList.remove("docs-dark");
      cart.style.visibility = "";
      var top = document.querySelector(".mast-mark") || cart;
      top.focus({ preventScroll:true });
      busy = false;
    });
  }

  window.addEventListener("resize", function(){
    if (!stage || busy) return;
    barTop = Math.min(barTop, innerHeight - BAR);
    stage.querySelectorAll(".doc-stage-bar, .doc-stage-front").forEach(function(el){ el.style.top = barTop + "px"; });
    place(0);
  });
  cart.addEventListener("click", eject);
  document.addEventListener("keydown", function(e){
    if (e.key === "Escape" && stage) insert();
  });
})();

(function(){
  "use strict";

  var KEY = "notice:slot-advance";
  var notice = document.getElementById("notice");
  if (!notice) return;
  try { if (localStorage.getItem(KEY) === "dismissed") return; } catch (e) {}
  notice.hidden = false;
  var root = document.documentElement.style;
  var fit = function(){ root.setProperty("--notice-h", notice.hidden ? "0px" : notice.offsetHeight + "px"); };
  fit();
  addEventListener("resize", fit);
  document.getElementById("notice-close").addEventListener("click", function(){
    try { localStorage.setItem(KEY, "dismissed"); } catch (e) {}
    notice.hidden = true;
    fit();
  });
})();
