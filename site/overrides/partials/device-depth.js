(function(){
  "use strict";

  var SVGNS = "http://www.w3.org/2000/svg";
  var AXIS_Y = 780, AXIS_Z = 23, BARREL_R = 40;
  var LID_VB = "0 0 820 725", DECK_VB = "0 725 820 830";

  var paintSuffix = "";
  function suffixed(markup){
    if (!paintSuffix) return markup;
    return markup
      .replace(/url\(#([\w-]+)\)/g, "url(#$1" + paintSuffix + ")")
      .replace(/href="#([\w-]+)"/g, 'href="#$1' + paintSuffix + '"');
  }
  function slice(host, vb, z, depthVar, units, inner){
    inner = suffixed(inner);
    var s = document.createElementNS(SVGNS, "svg");
    s.setAttribute("viewBox", vb);
    s.setAttribute("preserveAspectRatio", "none");
    s.innerHTML = inner;
    s.style.transform = "translateZ(calc(var(" + depthVar + ") * " + (z / units).toFixed(5) + "))";
    host.appendChild(s);
  }
  function deckBody(top){
    return '<path fill="url(#sideGrad)" d="M25,' + top + ' H795 A25,25 0 0 1 820,' + (top + 25) +
      ' V1475 A80,80 0 0 1 740,1555 H80 A80,80 0 0 1 0,1475 V' + (top + 25) +
      ' A25,25 0 0 1 25,' + top + ' Z"/>';
  }
  function barrel(p, h){
    var y0 = AXIS_Y - h, hh = 2 * h, w = p.x1 - p.x0;
    if (!p.dome) {
      return '<rect fill="url(#tubeGrad)" x="' + p.x0 + '" y="' + y0.toFixed(1) + '" width="' + w +
        '" height="' + hh.toFixed(1) + '" rx="' + Math.min(3, h * 0.5).toFixed(1) + '"/>';
    }
    var r = Math.min(h * 0.32, w * 0.5), yb = (y0 + hh).toFixed(1), yt = y0.toFixed(1);
    var d = p.dome === "L"
      ? "M" + (p.x0 + r) + "," + yt + " H" + p.x1 + " V" + yb + " H" + (p.x0 + r) +
        " A" + r.toFixed(1) + "," + h.toFixed(1) + " 0 0 1 " + (p.x0 + r) + "," + yt + " Z"
      : "M" + p.x0 + "," + yt + " H" + (p.x1 - r) + " A" + r.toFixed(1) + "," + h.toFixed(1) +
        " 0 0 1 " + (p.x1 - r) + "," + yb + " H" + p.x0 + " Z";
    return '<path fill="url(#capTube)" d="' + d + '"/>';
  }
  var HINGE = [
    { x0:0,   x1:63,  dome:"L" }, { x0:70,  x1:390 }, { x0:397, x1:688 },
    { x0:695, x1:750 }, { x0:757, x1:820, dome:"R" }
  ];
  var GROOVES = [[63, 70], [390, 397], [688, 695], [750, 757]];
  var RAISED = [
    { x:612, y:1026, g:"X" }, { x:533, y:1106, g:"Y" },
    { x:694, y:1106, g:"A" }, { x:612, y:1188, g:"B" }
  ];
  var RAISED_H = 14;
  function cross(attrs){
    return '<rect ' + attrs + ' x="166" y="1002" width="76" height="216" rx="16"/>' +
           '<rect ' + attrs + ' x="96" y="1074" width="216" height="72" rx="16"/>';
  }
  var SIDE_ART =
    '<svg viewBox="0 0 239 830" preserveAspectRatio="none">' +
    '<line x1="91" y1="40" x2="91" y2="790" stroke="rgba(0,0,0,.35)" stroke-width="1.6"/>' +
    '<ellipse cx="70" cy="344" rx="15" ry="36" fill="rgba(0,0,0,.28)"/>' +
    '<rect x="80" y="285" width="17" height="118" rx="7" fill="#0b0b0e"/>' +
    '<circle cx="86" cy="447" r="24" fill="rgba(0,0,0,.4)"/><circle cx="86" cy="447" r="19" fill="url(#btnGrad)"/>' +
    '<rect x="60" y="508" width="52" height="86" rx="26" fill="rgba(0,0,0,.4)"/>' +
    '<rect x="64" y="512" width="44" height="78" rx="22" fill="url(#btnGrad)"/>' +
    '<circle cx="182" cy="775" r="39" fill="url(#capGrad)" stroke="rgba(0,0,0,.25)" stroke-width="2"/>' +
    '</svg>';

  function addDepth(dev){
    var lid = dev.querySelector(".dv-lid"), deck = dev.querySelector(".dv-deck");
    if (!lid || !deck || lid.querySelector(".dv-depth")) return;
    paintSuffix = dev.classList.contains("is-peer") ? "-peer" : "";

    var ld = document.createElement("div");
    ld.className = "dv-depth";
    ld.setAttribute("aria-hidden", "true");
    for (var i = 1; i <= 9; i++) slice(ld, LID_VB, -40 * i / 10, "--depth-lid", 40, '<use href="#lid" fill="url(#sideGrad)"/>');
    slice(ld, LID_VB, 0.5, "--depth-lid", 40,
      '<rect x="72" y="722" width="316" height="15" fill="url(#sideGrad)"/>' +
      '<rect x="697" y="722" width="51" height="15" fill="url(#sideGrad)"/>');
    lid.insertBefore(ld, lid.firstChild);

    var dd = document.createElement("div");
    dd.className = "dv-depth";
    dd.setAttribute("aria-hidden", "true");
    for (var j = 1; j <= 20; j++) slice(dd, DECK_VB, -159 * j / 20, "--depth-deck", 159, deckBody(AXIS_Y));
    slice(dd, DECK_VB, AXIS_Z - 14, "--depth-deck", 159, '<rect x="20" y="744" width="780" height="72" fill="#121216"/>');
    var n = 28;
    for (var k = 1; k < n; k++) {
      var dz = BARREL_R - 2 * BARREL_R * k / n, h = Math.sqrt(BARREL_R * BARREL_R - dz * dz), inner = "";
      if (Math.abs(dz) < BARREL_R - 3) {
        var gh = Math.sqrt((BARREL_R - 3) * (BARREL_R - 3) - dz * dz);
        GROOVES.forEach(function(g){
          inner += '<rect fill="var(--shell-shade)" x="' + g[0] + '" y="' + (AXIS_Y - gh).toFixed(1) +
            '" width="' + (g[1] - g[0]) + '" height="' + (2 * gh).toFixed(1) + '"/>';
        });
      }
      HINGE.forEach(function(p){ inner += barrel(p, h); });
      slice(dd, DECK_VB, AXIS_Z + dz, "--depth-deck", 159, inner);
    }
    for (var m = 0; m <= 8; m++) {
      slice(dd, DECK_VB, -80 - 79 * m / 8, "--depth-deck", 159,
        '<rect fill="url(#btnGrad)" x="4" y="752" width="66" height="40" rx="12"/>' +
        '<rect fill="url(#btnGrad)" x="92" y="756" width="90" height="36" rx="8"/>' +
        '<rect fill="url(#btnGrad)" x="638" y="756" width="90" height="36" rx="8"/>' +
        '<rect fill="url(#btnGrad)" x="750" y="752" width="66" height="40" rx="12"/>');
    }
    for (var q = 1; q < 4; q++) {
      var wall = cross('fill="var(--btn)"') + cross('fill="rgba(0,0,0,.2)"');
      RAISED.forEach(function(b){
        wall += '<circle fill="var(--btn)" cx="' + b.x + '" cy="' + b.y + '" r="40"/>' +
                '<circle fill="rgba(0,0,0,.2)" cx="' + b.x + '" cy="' + b.y + '" r="40"/>';
      });
      slice(dd, DECK_VB, RAISED_H * q / 4, "--depth-deck", 159, wall);
    }
    var top = cross('fill="url(#btnGrad)"') + '<circle cx="204" cy="1110" r="17" class="dimple"/>';
    RAISED.forEach(function(b){
      top += '<circle cx="' + b.x + '" cy="' + b.y + '" r="40" fill="url(#btnGrad)" stroke="var(--etch)" stroke-opacity=".32" stroke-width="3"/>' +
             '<text x="' + b.x + '" y="' + (b.y + 10) + '" class="glyph" font-size="32">' + b.g + '</text>';
    });
    slice(dd, DECK_VB, RAISED_H, "--depth-deck", 159, top);
    deck.insertBefore(dd, deck.firstChild);

    ["lid-top", "lid-l", "lid-r"].forEach(function(c){
      var w = document.createElement("span");
      w.className = "dv-wall " + c;
      w.setAttribute("aria-hidden", "true");
      lid.appendChild(w);
    });
    ["deck-l", "deck-r", "deck-front"].forEach(function(c){
      var w = document.createElement("span");
      w.className = "dv-wall " + c;
      w.setAttribute("aria-hidden", "true");
      deck.appendChild(w);
    });

    var side = document.createElement("div");
    side.className = "dv-side";
    side.setAttribute("aria-hidden", "true");
    side.innerHTML = suffixed(SIDE_ART);
    deck.appendChild(side);
  }
  Array.prototype.forEach.call(document.querySelectorAll(".device"), addDepth);
  window.slotAddDepth = addDepth;
})();
