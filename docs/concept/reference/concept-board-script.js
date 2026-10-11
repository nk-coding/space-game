// Full script of the concept board (revision 16): part icon loader + drydock, sheets, compare, tech tree,
// evolution lab, galaxy generator, recipes, damage sandbox. Reference only: browser JS, placeholder numbers.


// Ship part icons, style 2a: drawn once as SVG images, then stamped onto the tiles by the canvas code below.
window.__shipIcons = {};
window.__shipIconsReady = (function () {
  var DEFS = "<defs><radialGradient id=\"gCore\"><stop offset=\"0\" stop-color=\"#fff4d6\"></stop><stop offset=\".4\" stop-color=\"#f2c472\" stop-opacity=\".9\"></stop><stop offset=\"1\" stop-color=\"#e2b15c\" stop-opacity=\"0\"></stop></radialGradient><radialGradient id=\"gCy\"><stop offset=\"0\" stop-color=\"#eafcff\"></stop><stop offset=\".45\" stop-color=\"#5cc6d6\" stop-opacity=\".9\"></stop><stop offset=\"1\" stop-color=\"#5cc6d6\" stop-opacity=\"0\"></stop></radialGradient><radialGradient id=\"gRed\"><stop offset=\"0\" stop-color=\"#ffe2da\"></stop><stop offset=\".45\" stop-color=\"#ec8f7c\" stop-opacity=\".9\"></stop><stop offset=\"1\" stop-color=\"#ec8f7c\" stop-opacity=\"0\"></stop></radialGradient><linearGradient id=\"gAmb\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" stop-color=\"#f6d595\"></stop><stop offset=\"1\" stop-color=\"#a87a35\"></stop></linearGradient><filter id=\"gl\" x=\"-60%\" y=\"-60%\" width=\"220%\" height=\"220%\"><feGaussianBlur stdDeviation=\"1.6\"></feGaussianBlur></filter></defs>";
  var SRC = {"fission":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><circle cx=\"10\" cy=\"10\" r=\"5\" fill=\"#e2b15c\"></circle></g><circle cx=\"10\" cy=\"10\" r=\"5.2\" fill=\"url(#gCore)\"></circle><circle cx=\"10\" cy=\"10\" r=\"4.6\" fill=\"none\" stroke=\"#e2b15c\" stroke-width=\".9\"></circle><circle cx=\"10\" cy=\"10\" r=\"1.5\" fill=\"#fff4d6\"></circle>"],"fusion":[2,2,"<g filter=\"url(#gl)\" opacity=\"0.55\"><circle cx=\"20\" cy=\"20\" r=\"12\" fill=\"none\" stroke=\"#e2b15c\" stroke-width=\"2.4\"></circle><circle cx=\"20\" cy=\"20\" r=\"6\" fill=\"#e2b15c\"></circle></g><circle cx=\"20\" cy=\"20\" r=\"13\" fill=\"url(#gCore)\" opacity=\".55\"></circle><circle cx=\"20\" cy=\"20\" r=\"12\" fill=\"none\" stroke=\"#e2b15c\" stroke-width=\"1.5\"></circle><circle cx=\"20\" cy=\"20\" r=\"7\" fill=\"none\" stroke=\"#f2c472\" stroke-width=\"1\"></circle><circle cx=\"20\" cy=\"20\" r=\"3.2\" fill=\"#fff4d6\"></circle><g fill=\"url(#gAmb)\" stroke=\"#e2b15c\" stroke-width=\".6\"><rect x=\"18.5\" y=\"4.6\" width=\"3\" height=\"4\"></rect><rect x=\"18.5\" y=\"31.4\" width=\"3\" height=\"4\"></rect><rect x=\"4.6\" y=\"18.5\" width=\"4\" height=\"3\"></rect><rect x=\"31.4\" y=\"18.5\" width=\"4\" height=\"3\"></rect></g>"],"cap":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><g fill=\"#e2b15c\"><rect x=\"5\" y=\"6.5\" width=\"2.6\" height=\"7\"></rect><rect x=\"8.7\" y=\"6.5\" width=\"2.6\" height=\"7\"></rect><rect x=\"12.4\" y=\"6.5\" width=\"2.6\" height=\"7\"></rect></g></g><g fill=\"url(#gAmb)\" stroke=\"#e2b15c\" stroke-width=\".5\"><rect x=\"5\" y=\"6.5\" width=\"2.6\" height=\"7\" rx=\".6\"></rect><rect x=\"8.7\" y=\"6.5\" width=\"2.6\" height=\"7\" rx=\".6\"></rect><rect x=\"12.4\" y=\"6.5\" width=\"2.6\" height=\"7\" rx=\".6\"></rect></g>"],"pulse":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><circle cx=\"10\" cy=\"11\" r=\"4\" fill=\"#5cc6d6\"></circle></g><circle cx=\"10\" cy=\"11\" r=\"4.6\" fill=\"url(#gCy)\"></circle><circle cx=\"10\" cy=\"11\" r=\"3.6\" fill=\"none\" stroke=\"#5cc6d6\" stroke-width=\".9\"></circle><path d=\"M10 7.4V2\" stroke=\"#5cc6d6\" stroke-opacity=\".7\" stroke-width=\"1.2\"></path>"],"beam":[1,2,"<g filter=\"url(#gl)\" opacity=\"0.55\"><circle cx=\"10\" cy=\"30\" r=\"5\" fill=\"#5cc6d6\"></circle><rect x=\"8.5\" y=\"2\" width=\"3\" height=\"22\" fill=\"#5cc6d6\" opacity=\".5\"></rect></g><rect x=\"8.3\" y=\"2\" width=\"3.4\" height=\"23\" rx=\"1\" fill=\"#5cc6d6\" fill-opacity=\".22\" stroke=\"#5cc6d6\" stroke-opacity=\".7\" stroke-width=\".6\"></rect><circle cx=\"10\" cy=\"30\" r=\"6\" fill=\"url(#gCy)\"></circle><circle cx=\"10\" cy=\"30\" r=\"4.8\" fill=\"none\" stroke=\"#5cc6d6\" stroke-width=\".9\"></circle><circle cx=\"10\" cy=\"30\" r=\"2\" fill=\"none\" stroke=\"#eafcff\" stroke-width=\".8\"></circle>"],"shield":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><path d=\"M5.5 13a4.5 4.5 0 0 1 9 0M3.8 13a6.2 6.2 0 0 1 12.4 0\" fill=\"none\" stroke=\"#8fb0f2\" stroke-width=\"1.4\"></path></g><path d=\"M5.5 13a4.5 4.5 0 0 1 9 0M3.8 13a6.2 6.2 0 0 1 12.4 0\" fill=\"none\" stroke=\"#a9c0f4\" stroke-width=\".9\"></path><circle cx=\"10\" cy=\"13.4\" r=\"1.6\" fill=\"#dfe8ff\"></circle>"],"rad":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><path d=\"M5.5 4.5v11M8.5 4.5v11M11.5 4.5v11M14.5 4.5v11\" stroke=\"#ec8f7c\" stroke-width=\"1.6\"></path></g><path d=\"M5.5 4.5v11M8.5 4.5v11M11.5 4.5v11M14.5 4.5v11\" stroke=\"#f4a998\" stroke-width=\"1.1\" stroke-linecap=\"round\"></path>"],"thruster":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><ellipse cx=\"10\" cy=\"16.5\" rx=\"4.5\" ry=\"1.6\" fill=\"#8cefdb\"></ellipse></g><rect x=\"7.5\" y=\"3.5\" width=\"5\" height=\"3.5\" rx=\".8\" fill=\"none\" stroke=\"#8cefdb\" stroke-width=\".7\"></rect><path d=\"M8 7h4l3 10H5z\" fill=\"#8cefdb\" fill-opacity=\".12\" stroke=\"#8cefdb\" stroke-width=\".8\"></path><ellipse cx=\"10\" cy=\"16.6\" rx=\"4.4\" ry=\"1.1\" fill=\"#e0fff7\"></ellipse>"],"torch":[2,2,"<g filter=\"url(#gl)\" opacity=\"0.55\"><ellipse cx=\"20\" cy=\"36\" rx=\"12\" ry=\"2.6\" fill=\"#8cefdb\"></ellipse></g><rect x=\"15\" y=\"5\" width=\"10\" height=\"9\" rx=\"1.2\" fill=\"none\" stroke=\"#8cefdb\" stroke-width=\".8\"></rect><path d=\"M15 14h10l8 22H7z\" fill=\"#8cefdb\" fill-opacity=\".12\" stroke=\"#8cefdb\" stroke-width=\".9\"></path><path d=\"M11.4 25h17.2M9.6 30h20.8\" stroke=\"#8cefdb\" stroke-opacity=\".55\" stroke-width=\".8\"></path><ellipse cx=\"20\" cy=\"36\" rx=\"12.5\" ry=\"1.8\" fill=\"#e0fff7\"></ellipse>"],"cargo":[1,1,"<g fill=\"#8d9db4\" fill-opacity=\".14\" stroke=\"#8d9db4\" stroke-opacity=\".75\" stroke-width=\".7\"><rect x=\"4.5\" y=\"4.5\" width=\"5\" height=\"5\" rx=\".6\"></rect><rect x=\"10.5\" y=\"4.5\" width=\"5\" height=\"5\" rx=\".6\"></rect><rect x=\"4.5\" y=\"10.5\" width=\"5\" height=\"5\" rx=\".6\"></rect><rect x=\"10.5\" y=\"10.5\" width=\"5\" height=\"5\" rx=\".6\"></rect></g>"],"core":[1,1,"<g filter=\"url(#gl)\" opacity=\"0.55\"><rect x=\"7\" y=\"7\" width=\"6\" height=\"6\" fill=\"#ae8cec\"></rect></g><path d=\"M8.5 7V4.6M11.5 7V4.6M8.5 13v2.4M11.5 13v2.4M7 8.5H4.6M7 11.5H4.6M13 8.5h2.4M13 11.5h2.4\" stroke=\"#ae8cec\" stroke-width=\".8\"></path><rect x=\"7\" y=\"7\" width=\"6\" height=\"6\" fill=\"#ae8cec\" fill-opacity=\".35\" stroke=\"#c9b2f5\" stroke-width=\".8\"></rect><rect x=\"9\" y=\"9\" width=\"2\" height=\"2\" fill=\"#efe6ff\"></rect>"],"hold":[2,2,"<g fill=\"#8d9db4\" fill-opacity=\".14\" stroke=\"#8d9db4\" stroke-opacity=\".75\" stroke-width=\".8\"><rect x=\"6\" y=\"6\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"15.7\" y=\"6\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"25.4\" y=\"6\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"6\" y=\"15.7\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"15.7\" y=\"15.7\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"25.4\" y=\"15.7\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"6\" y=\"25.4\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"15.7\" y=\"25.4\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect><rect x=\"25.4\" y=\"25.4\" width=\"8.6\" height=\"8.6\" rx=\".8\"></rect></g>"]};
  var jobs = Object.keys(SRC).map(function (k) {
    return new Promise(function (res) {
      var w = SRC[k][0], h = SRC[k][1], img = new Image();
      img.onload = img.onerror = function () { res(); };
      img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="' + w * 80 + '" height="' + h * 80 + '" viewBox="0 0 ' + w * 20 + ' ' + h * 20 + '">' + DEFS + SRC[k][2] + '</svg>');
      window.__shipIcons[k] = img;
    });
  });
  return Promise.race([Promise.all(jobs), new Promise(function (res) { setTimeout(res, 1500); })]);
})();

// ---- next <script> ----

(window.__shipIconsReady || Promise.resolve()).then(function () {
// MODEL START
const W = 16, H = 12;
const MATS = {
  steel: { name: 'Steel', mass: 2, hp: 1 },
  composite: { name: 'Composite', mass: 1.5, hp: 1.4 },
  reactive: { name: 'Reactive', mass: 2.4, hp: 2.1 },
};
const PARTS = {
  block: { name: 'Armor block', short: 'Block', group: 'Hull', cat: 'steel', w: 1, h: 1, shape: 'rect', hull: true, desc: 'Plain plating. Material sets weight and toughness.' },
  slope1: { name: 'Slope 1×1', short: 'Slope 1×1', group: 'Hull', cat: 'steel', w: 1, h: 1, shape: 'slope', hull: true, desc: '45° slope. Orients itself to the hull; click it again to turn it.' },
  slope2: { name: 'Slope 1×2', short: 'Slope 1×2', group: 'Hull', cat: 'steel', w: 2, h: 1, shape: 'slope', hull: true, desc: 'Shallow slope over two cells. Click again to turn or flip it.' },
  slope3: { name: 'Slope 1×3', short: 'Slope 1×3', group: 'Hull', cat: 'steel', w: 3, h: 1, shape: 'slope', hull: true, desc: 'Long, flat slope over three cells. Good for noses and wing edges.' },
  fission: { name: 'Fission reactor', short: 'Fission', group: 'Power', cat: 'amber', w: 1, h: 1, mass: 4, out: 10, heat: 3, limit: 6, desc: '10 power. +2 for each venting radiator beside it.' },
  fusion: { name: 'Fusion reactor', short: 'Fusion 2×2', group: 'Power', cat: 'amber', w: 2, h: 2, mass: 12, out: 30, heat: 6, limit: 8, desc: '2×2. 30 power, +2 per venting radiator. Runs hot.' },
  cap: { name: 'Capacitor', short: 'Capacitor', group: 'Power', cat: 'amber', w: 1, h: 1, mass: 1, bonus: 0.15, heat: 0.15, limit: 4, desc: 'Lasers beside it fire 15% faster, up to 3 per laser.' },
  pulse: { name: 'Pulse laser', short: 'Pulse laser', group: 'Combat', cat: 'cyan', w: 1, h: 1, mass: 2, draw: 4, dmg: 1, heat: 1.5, limit: 3, gun: true, desc: 'Fixed forward gun. Needs a clear line of fire.' },
  beam: { name: 'Beam laser', short: 'Beam 1×2', group: 'Combat', cat: 'cyan', w: 1, h: 2, mass: 4, draw: 9, dmg: 2.6, heat: 3.2, limit: 4.2, gun: true, desc: '1×2. Continuous beam: strong, hungry, hot.' },
  shield: { name: 'Shield', short: 'Shield', group: 'Combat', cat: 'shield', w: 1, h: 1, mass: 2, draw: 3, heat: 0.8, limit: 3.2, desc: 'Projects a bubble over the hull around it.' },
  rad: { name: 'Radiator', short: 'Radiator', group: 'Heat and thrust', cat: 'coral', w: 1, h: 1, mass: 1, vent: 1.5, heat: 0, desc: 'Dumps heat, but only through edges open to space.' },
  thruster: { name: 'Thruster', short: 'Thruster', group: 'Heat and thrust', cat: 'mint', w: 1, h: 1, mass: 2, draw: 2, thrust: 12, heat: 0.8, limit: 3.2, engine: true, desc: '12 thrust. Needs open space behind it.' },
  torch: { name: 'Torch drive', short: 'Torch 2×2', group: 'Heat and thrust', cat: 'mint', w: 2, h: 2, mass: 9, draw: 6, thrust: 45, heat: 4, limit: 5, engine: true, desc: '2×2. 45 thrust. Hot, heavy, and very fast.' },
  cargo: { name: 'Cargo bay', short: 'Cargo bay', group: 'Other', cat: 'steel', w: 1, h: 1, mass: 1.5, cargo: 4, desc: 'Holds 4 units of salvage. Hold space competes with everything else.' },
  hold: { name: 'Cargo hold', short: 'Hold 2×2', group: 'Other', cat: 'steel', w: 2, h: 2, mass: 5, cargo: 20, desc: '2×2. Holds 20 units of salvage. Bulky, but cheaper per unit.' },
  core: { name: 'Logic core', short: 'Logic core', group: 'Other', cat: 'violet', w: 1, h: 1, mass: 1, draw: 1, slots: 6, heat: 0.4, limit: 3, desc: 'Runs behaviors: 6 rule slots per core.' },
};
const DIRS = [[0, -1], [1, 0], [0, 1], [-1, 0]]; // up, right, down, left

function dims(p) { const d = PARTS[p.type]; return p.rot % 2 ? [d.h, d.w] : [d.w, d.h]; }
function xfCell(i, j, w, h, rot, flip) { // local cell -> rotated/flipped local cell
  if (flip) i = w - 1 - i;
  for (let r = 0; r < rot; r++) { const ni = h - 1 - j, nj = i; i = ni; j = nj; const t = w; w = h; h = t; }
  return [i, j];
}
function xfDir(d, rot, flip) { if (flip && d % 2 === 1) d = 4 - d; return (d + rot) % 4; }
function xfPoint(x, y, w, h, rot, flip) {
  if (flip) x = w - x;
  for (let r = 0; r < rot; r++) { const nx = h - y, ny = x; x = nx; y = ny; const t = w; w = h; h = t; }
  return [x, y];
}
function partCells(p) {
  const d = PARTS[p.type], out = [];
  for (let j = 0; j < d.h; j++) for (let i = 0; i < d.w; i++) { const [a, b] = xfCell(i, j, d.w, d.h, p.rot, p.flip); out.push([p.x + a, p.y + b]); }
  return out;
}
function fullEdges(p) { // set of "x,y,d" boundary edges that are straight, solid sides
  const d = PARTS[p.type], set = new Set();
  const add = (i, j, dir) => { const [a, b] = xfCell(i, j, d.w, d.h, p.rot, p.flip); set.add((p.x + a) + ',' + (p.y + b) + ',' + xfDir(dir, p.rot, p.flip)); };
  if (d.shape === 'slope') {
    for (let j = 0; j < d.h; j++) add(0, j, 3);
    for (let i = 0; i < d.w; i++) add(i, d.h - 1, 2);
  } else {
    for (let i = 0; i < d.w; i++) { add(i, 0, 0); add(i, d.h - 1, 2); }
    for (let j = 0; j < d.h; j++) { add(0, j, 3); add(d.w - 1, j, 1); }
  }
  return set;
}
function polygon(p) { // absolute grid-unit polygon
  const d = PARTS[p.type];
  const pts = d.shape === 'slope' ? [[0, 0], [0, d.h], [d.w, d.h]] : [[0, 0], [d.w, 0], [d.w, d.h], [0, d.h]];
  return pts.map(([x, y]) => { const [a, b] = xfPoint(x, y, d.w, d.h, p.rot, p.flip); return [p.x + a, p.y + b]; });
}
function slopeNormal(p) { // outward normal of the slope face
  const d = PARTS[p.type];
  let [x, y] = [d.h, -d.w]; // perpendicular to the hypotenuse (0,0)-(w,h), pointing away from the right-angle corner
  if (p.flip) x = -x;
  for (let r = 0; r < p.rot; r++) { const nx = -y, ny = x; x = nx; y = ny; }
  const L = Math.hypot(x, y); return [x / L, y / L];
}
function area(p) { const d = PARTS[p.type]; return d.shape === 'slope' ? d.w * d.h / 2 : d.w * d.h; }
function pv(p, key) { // part value
  const d = PARTS[p.type];
  if (d.hull && (key === 'mass' || key === 'hp')) return MATS[p.mat || 'steel'][key] * area(p);
  return d[key] || 0;
}
function fits(p) { const [w, h] = dims(p); return p.x >= 0 && p.y >= 0 && p.x + w <= W && p.y + h <= H; }
function buildOcc(parts) {
  const occ = Array.from({ length: H }, () => Array(W).fill(-1));
  parts.forEach((p, i) => partCells(p).forEach(([x, y]) => { if (x >= 0 && y >= 0 && x < W && y < H) occ[y][x] = i; }));
  return occ;
}
function makeCtx(parts) { return { parts, occ: buildOcc(parts), edges: parts.map(fullEdges) }; }
function occAt(C, x, y) { return x < 0 || y < 0 || x >= W || y >= H ? -1 : C.occ[y][x]; }
function linked(C, x, y, d) {
  const a = occAt(C, x, y), nx = x + DIRS[d][0], ny = y + DIRS[d][1], b = occAt(C, nx, ny);
  if (a < 0 || b < 0) return false;
  if (a === b) return true;
  return C.edges[a].has(x + ',' + y + ',' + d) && C.edges[b].has(nx + ',' + ny + ',' + ((d + 2) % 4));
}
function exposure(C, x, y) {
  const a = occAt(C, x, y); if (a < 0) return 0;
  let n = 0;
  for (let d = 0; d < 4; d++) {
    const b = occAt(C, x + DIRS[d][0], y + DIRS[d][1]);
    if (b === a) continue;
    if (C.edges[a].has(x + ',' + y + ',' + d) && !linked(C, x, y, d)) n++;
  }
  if (PARTS[C.parts[a].type].shape === 'slope') n++;
  return n;
}

// orientation helpers for slopes
function placementScore(C, p) {
  if (!fits(p)) return -999;
  const cells = partCells(p), mine = new Set(cells.map(c => c.join(','))), fe = fullEdges(p);
  let sc = 0;
  for (const [x, y] of cells) {
    if (occAt(C, x, y) >= 0) sc -= 5;
    for (let d = 0; d < 4; d++) {
      const nx = x + DIRS[d][0], ny = y + DIRS[d][1];
      if (mine.has(nx + ',' + ny)) continue;
      const b = occAt(C, nx, ny);
      const isFull = fe.has(x + ',' + y + ',' + d);
      if (b >= 0 && isFull && C.edges[b].has(nx + ',' + ny + ',' + ((d + 2) % 4))) sc += 2;
      else if (b >= 0 && !isFull) sc -= 1;
    }
  }
  const n = slopeNormal(p), [w] = dims(p), mid = p.x + w / 2, side = mid < W / 2 - 0.01 ? -1 : mid > W / 2 + 0.01 ? 1 : 0;
  sc += 0.1 * n[0] * side - 0.05 * n[1];
  return sc;
}
function bestOrientation(C, type, x, y) {
  let best = null, bestSc = -Infinity;
  for (let flip = 0; flip < 2; flip++) for (let rot = 0; rot < 4; rot++) {
    const p = { type, x, y, rot, flip: !!flip };
    const sc = placementScore(C, p);
    if (sc > bestSc) { bestSc = sc; best = p; }
  }
  return best;
}
function mirrorPart(p) {
  const [w] = dims(p);
  return { type: p.type, x: W - p.x - w, y: p.y, rot: (4 - p.rot) % 4, flip: !p.flip, mat: p.mat };
}
function bevel(parts) {
  const C = makeCtx(parts);
  return parts.map((p, i) => {
    if (p.type !== 'block') return p;
    const [x, y] = [p.x, p.y];
    const con = [0, 1, 2, 3].map(d => linked(C, x, y, d));
    if (con.filter(Boolean).length !== 2) return p;
    for (let flip = 0; flip < 2; flip++) for (let rot = 0; rot < 4; rot++) {
      const q = { type: 'slope1', x, y, rot, flip: !!flip, mat: p.mat };
      const fe = fullEdges(q);
      if ([0, 1, 2, 3].every(d => fe.has(x + ',' + y + ',' + d) === con[d])) return q;
    }
    return p;
  });
}

function analyze(parts, iters) {
  const C = makeCtx(parts);
  const cellPiece = Array.from({ length: H }, () => Array(W).fill(-1));
  const pieces = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (C.occ[y][x] < 0 || cellPiece[y][x] >= 0) continue;
    const id = pieces.length, stack = [[x, y]], partSet = new Set();
    cellPiece[y][x] = id;
    while (stack.length) {
      const [cx, cy] = stack.pop(); partSet.add(C.occ[cy][cx]);
      for (let d = 0; d < 4; d++) {
        const nx = cx + DIRS[d][0], ny = cy + DIRS[d][1];
        if (linked(C, cx, cy, d) && cellPiece[ny][nx] < 0) { cellPiece[ny][nx] = id; stack.push([nx, ny]); }
      }
    }
    pieces.push({ id, parts: [...partSet], out: 0, draw: 0, ratio: 1, load: 0, hasCore: false });
  }
  const partPiece = parts.map(p => { const [x, y] = partCells(p)[0]; return cellPiece[y][x]; });
  const neighborsOf = i => {
    const set = new Set();
    for (const [x, y] of partCells(parts[i])) for (let d = 0; d < 4; d++) {
      const b = occAt(C, x + DIRS[d][0], y + DIRS[d][1]);
      if (b >= 0 && b !== i && linked(C, x, y, d)) set.add(b);
    }
    return [...set];
  };
  const venting = i => partCells(parts[i]).some(([x, y]) => exposure(C, x, y) > 0);
  const links = [], laserBonus = {};
  parts.forEach((p, i) => {
    const pc = pieces[partPiece[i]], d = PARTS[p.type];
    if (p.type === 'core') pc.hasCore = true;
    pc.draw += d.draw || 0;
    if (d.out) {
      let rads = 0;
      neighborsOf(i).forEach(b => { if (parts[b].type === 'rad' && venting(b)) { rads++; links.push({ a: i, b, kind: 'rad' }); } });
      pc.out += d.out + 2 * rads;
    }
    if (d.gun) {
      let bonus = 0, n = 0;
      neighborsOf(i).forEach(b => { if (parts[b].type === 'cap' && n < 3) { n++; bonus += PARTS.cap.bonus; links.push({ a: i, b, kind: 'cap' }); } });
      laserBonus[i] = bonus;
    }
  });
  pieces.forEach(pc => { pc.ratio = pc.draw === 0 ? 1 : Math.min(1, pc.out / pc.draw); pc.load = pc.out === 0 ? 0 : Math.min(1, pc.draw / pc.out); });
  let main = null;
  for (const pc of pieces) {
    const size = pc.parts.length;
    if (!main || (pc.hasCore && !main.hasCore) || (pc.hasCore === main.hasCore && size > main.parts.length)) main = pc;
  }
  // heat per cell
  const gen = Array.from({ length: H }, () => Array(W).fill(0)), cool = gen.map(r => r.slice()), T = gen.map(r => r.slice());
  parts.forEach((p, i) => {
    const d = PARTS[p.type], pc = pieces[partPiece[i]], cells = partCells(p);
    let g = d.heat || 0;
    if (d.out) g = pc.draw ? g * (1 + 2 * pc.load) / 3 : 0;
    else if (d.draw) g *= pc.ratio;
    cells.forEach(([x, y]) => {
      gen[y][x] = g / cells.length;
      cool[y][x] = 0.02 + exposure(C, x, y) * (p.type === 'rad' ? d.vent : 0.3);
    });
  });
  const nIt = iters || 400;
  for (let it = 0; it < nIt; it++) {
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      if (C.occ[y][x] < 0) continue;
      let sum = 0, n = 0;
      for (let d = 0; d < 4; d++) if (linked(C, x, y, d)) { sum += T[y + DIRS[d][1]][x + DIRS[d][0]]; n++; }
      T[y][x] = (gen[y][x] + sum) / (n + cool[y][x]);
    }
  }
  const temp = parts.map(p => Math.max(...partCells(p).map(([x, y]) => T[y][x])));
  const s = { mass: 0, cx: 0, cy: 0, thrust: 0, tx: 0, firepower: 0, guns: 0, blocked: [], slots: 0, armor: 0, cargo: 0, hot: [], engines: [], peak: 0 };
  if (main) {
    const mp = new Set(main.parts);
    main.parts.forEach(i => {
      const p = parts[i], m = pv(p, 'mass'), cells = partCells(p);
      s.mass += m; s.armor += pv(p, 'hp');
      cells.forEach(([x, y]) => { s.cx += m / cells.length * (x + 0.5); s.cy += m / cells.length * (y + 0.5); });
    });
    s.cx /= s.mass; s.cy /= s.mass;
    main.parts.forEach(i => {
      const p = parts[i], d = PARTS[p.type], cells = partCells(p);
      const over = d.limit ? temp[i] > d.limit : false;
      if (d.limit) s.peak = Math.max(s.peak, temp[i] / d.limit);
      if (over) s.hot.push(i);
      if (d.engine) {
        const [w, h] = dims(p);
        let open = true;
        for (let k = 0; k < w; k++) if (occAt(C, p.x + k, p.y + h) >= 0) open = false;
        const f = open && !over ? d.thrust * main.ratio : 0;
        s.engines.push({ i, open, f });
        s.thrust += f; s.tx += f * (p.x + w / 2);
      }
      if (d.gun) {
        s.guns++;
        const [w] = dims(p);
        let clear = true;
        for (let k = 0; k < w; k++) for (let yy = p.y - 1; yy >= 0; yy--) if (C.occ[yy][p.x + k] >= 0) clear = false;
        if (!clear) s.blocked.push(i);
        if (clear && !over) s.firepower += d.dmg * (1 + (laserBonus[i] || 0)) * main.ratio;
      }
      if (d.slots && main.ratio > 0) s.slots += d.slots;
      if (d.cargo) s.cargo += d.cargo;
    });
    s.tx = s.thrust ? s.tx / s.thrust : s.cx;
  }
  return { C, pieces, partPiece, main, links, T, temp, s };
}

// samples (type, x, y, rot, flip, mat)
function P(type, x, y, rot, flip, mat) { return { type, x, y, rot: rot || 0, flip: !!flip, mat: mat || 'steel' }; }
function withMirror(list) {
  const out = [];
  list.forEach(p => { out.push(p); const m = mirrorPart(p); if (m.x !== p.x) out.push(m); });
  return out;
}
const SAMPLE_MID = () => withMirror([
  P('beam', 7, 1),
  P('slope3', 4, 1, 0, true, 'composite'),
  P('pulse', 1, 2), P('slope2', 2, 2, 0, true, 'composite'), P('block', 4, 2, 0, false, 'composite'), P('block', 5, 2, 0, false, 'composite'), P('cap', 6, 2),
  P('cap', 1, 3), P('rad', 2, 3), P('fission', 3, 3), P('shield', 4, 3), P('core', 5, 3), P('block', 6, 3, 0, false, 'composite'),
  P('rad', 3, 4), P('block', 4, 4, 0, false, 'composite'), P('cargo', 5, 4), P('block', 6, 4, 0, false, 'composite'),
  P('slope1', 4, 5, 2, false, 'composite'), P('thruster', 5, 5), P('rad', 6, 5),
]).concat([P('fusion', 7, 3), P('torch', 7, 5)]);
const SAMPLE_EARLY = () => withMirror([
  P('slope1', 6, 1, 0, true), P('pulse', 7, 1),
  P('slope1', 5, 2, 0, true), P('block', 6, 2), P('cap', 7, 2),
  P('rad', 4, 3), P('fission', 5, 3), P('cargo', 6, 3), P('core', 7, 3),
  P('slope1', 4, 4, 2, false), P('thruster', 5, 4), P('shield', 6, 4), P('block', 7, 4),
]);

// ---------- toy evolution (the real arena scores designs by fights) ----------
const COST = { block: 1, slope1: 0.5, slope2: 1, slope3: 1.5, fission: 4, fusion: 10, cap: 2, pulse: 3, beam: 7, shield: 3, rad: 2, thruster: 3, torch: 9, core: 3, cargo: 1.5, hold: 5 };
const MAT_COST = { steel: 1, composite: 1.5, reactive: 2 };
const BUDGET = 60;
function cost(p) { return COST[p.type] * (PARTS[p.type].hull ? MAT_COST[p.mat || 'steel'] : 1); }
function shipCost(list) { return list.reduce((t, p) => t + cost(p), 0); }
function makeRng(seed) { let s = (seed >>> 0) || 1; return () => { s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0; return s / 4294967296; }; }
const isCenter = p => { const [w] = dims(p); return !PARTS[p.type].hull && w === 2 && p.x === 7; };
function expand(half) { const out = []; half.forEach(p => { out.push(p); if (!isCenter(p)) out.push(mirrorPart(p)); }); return out; }
function inHalf(p) { const [w, h] = dims(p); if (p.y < 0 || p.y + h > H) return false; return isCenter(p) || (p.x >= 0 && p.x + w <= 8); }
function cellsFree(C, p) { return partCells(p).every(([x, y]) => x >= 0 && y >= 0 && x < W && y < H && C.occ[y][x] < 0); }
function validNew(half, p) {
  if (!inHalf(p) || !fits(p)) return false;
  const C = makeCtx(expand(half));
  if (!cellsFree(C, p)) return false;
  if (!isCenter(p)) { const m = mirrorPart(p); if (!cellsFree(C, m)) return false; const own = new Set(partCells(p).map(c => c.join(','))); if (partCells(m).some(c => own.has(c.join(',')))) return false; }
  return true;
}
const ADD_W = [['block', 18], ['slope1', 8], ['slope2', 5], ['slope3', 3], ['fission', 8], ['fusion', 3], ['cap', 7], ['pulse', 10], ['beam', 5], ['shield', 6], ['rad', 8], ['thruster', 8], ['torch', 3], ['core', 3]];
const MATS_L = ['steel', 'composite', 'reactive'];
function pickW(R, list) { const tot = list.reduce((t, e) => t + e[1], 0); let r = R() * tot; for (const e of list) { r -= e[1]; if (r <= 0) return e[0]; } return list[0][0]; }
function mutAdd(half, R) {
  const ship = expand(half), C = makeCtx(ship), cand = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < 8; x++) {
    if (C.occ[y][x] >= 0) continue;
    if (!ship.length || DIRS.some(([dx, dy]) => { const nx = x + dx, ny = y + dy; return nx >= 0 && ny >= 0 && nx < W && ny < H && C.occ[ny][nx] >= 0; })) cand.push([x, y]);
  }
  if (!cand.length) return half;
  const [cx, cy] = cand[Math.floor(R() * cand.length)], type = pickW(R, ADD_W), d = PARTS[type];
  let p;
  if (!d.hull && d.w === 2) p = { type, x: 7, y: Math.min(cy, H - d.h), rot: 0, flip: false, mat: 'steel' };
  else if (d.shape === 'slope') {
    let best = null, bs = -Infinity;
    for (let k = 0; k < 8; k++) { const q = { type, x: cx, y: cy, rot: k % 4, flip: k >= 4, mat: MATS_L[Math.floor(R() * 3)] }; const [w, h] = dims(q); q.x = Math.max(0, Math.min(cx - (w - 1), 8 - w)); const sc = validNew(half, q) ? placementScore(C, q) + R() * 0.5 : -Infinity; if (sc > bs) { bs = sc; best = q; } }
    p = best;
  } else p = { type, x: Math.max(0, Math.min(cx, 8 - d.w)), y: Math.min(cy, H - d.h), rot: 0, flip: false, mat: d.hull ? MATS_L[Math.floor(R() * 3)] : 'steel' };
  if (p && validNew(half, p)) return half.concat([p]);
  return half;
}
function mutate(half, R) {
  const r = R(); half = half.map(p => Object.assign({}, p));
  if (r < 0.45 || half.length < 3) return mutAdd(half, R);
  const i = Math.floor(R() * half.length), p = half[i], d = PARTS[p.type];
  if (r < 0.65) { half.splice(i, 1); return half; }
  if (r < 0.8) {
    if (d.hull) { p.mat = MATS_L[Math.floor(R() * 3)]; return half; }
    if (d.w === 1 && d.h === 1) { const opts = ['fission', 'cap', 'pulse', 'shield', 'rad', 'thruster', 'core', 'block']; p.type = opts[Math.floor(R() * opts.length)]; p.mat = 'steel'; return half; }
    return half;
  }
  const rest = half.filter((_, j) => j !== i), q = Object.assign({}, p);
  if (R() < 0.5) q.x += R() < 0.5 ? -1 : 1; else q.y += R() < 0.5 ? -1 : 1;
  if (validNew(rest, q)) return rest.concat([q]);
  return half;
}
function crossover(a, b, R) { const k = 1 + Math.floor(R() * 8); return a.filter(p => p.y < k).concat(b.filter(p => p.y >= k)).map(p => Object.assign({}, p)); }
function repair(half, R) {
  let out = [];
  half.forEach(p => { if (validNew(out, p)) out.push(p); });
  const r = analyze(expand(out), 1);
  if (r.main) { const keep = new Set(); const ex = expand(out); r.main.parts.forEach(i => keep.add(ex[i])); out = out.filter(p => keep.has(p)); }
  while (shipCost(expand(out)) > BUDGET && out.length > 1) { const cands = out.map((p, i) => i).filter(i => out[i].type !== 'core'); const j = cands.length ? cands[Math.floor(R() * cands.length)] : 0; out.splice(j, 1); }
  return out;
}
function toyScore(list) {
  if (!list.length) return { score: 0 };
  const r = analyze(list, 120);
  if (!r.main) return { score: 0, r };
  const s = r.s, shields = r.main.parts.filter(i => list[i].type === 'shield').length;
  const fire = s.firepower, tough = s.armor + 2 * shields, agility = Math.min(1, s.thrust / Math.max(1, s.mass));
  const yaw = Math.abs(s.tx - s.cx) > 0.1 ? 0.6 : 1, brain = s.slots > 0 ? 1 : 0.3;
  return { score: 10 * Math.sqrt(Math.max(0, fire * tough * agility)) * yaw * brain, fire, tough, agility, r };
}
function seedHalf() { return SAMPLE_EARLY().filter(p => p.x + dims(p)[0] <= 8 && p.x < 8); }
function evoInit(seed, size) {
  const R = makeRng(seed), pop = [];
  for (let i = 0; i < size; i++) { let g = seedHalf(); const n = i === 0 ? 0 : 1 + Math.floor(R() * 6); for (let k = 0; k < n; k++) g = mutate(g, R); g = repair(g, R); pop.push({ g, f: toyScore(expand(g)).score }); }
  return { R, pop, gen: 0, history: [] };
}
function evoStep(E) {
  const R = E.R, pop = E.pop.slice().sort((a, b) => b.f - a.f), next = pop.slice(0, 2);
  const pick = () => { let best = null; for (let k = 0; k < 3; k++) { const c = pop[Math.floor(R() * pop.length)]; if (!best || c.f > best.f) best = c; } return best; };
  while (next.length < pop.length) {
    let g = R() < 0.6 ? crossover(pick().g, pick().g, R) : pick().g.map(p => Object.assign({}, p));
    const n = 1 + Math.floor(R() * 3); for (let k = 0; k < n; k++) g = mutate(g, R);
    g = repair(g, R);
    next.push({ g, f: toyScore(expand(g)).score });
  }
  E.pop = next; E.gen++;
  const best = next.reduce((a, b) => (b.f > a.f ? b : a));
  E.history.push(best.f); E.best = best;
  return E;
}
// MODEL END

  const root = getComputedStyle(document.documentElement);
  const COL = {};
  ['steel', 'amber', 'coral', 'cyan', 'shield', 'mint', 'violet', 'ink', 'dim', 'faint', 'line', 'line-soft', 'hull', 'void', 'chart-line',
    'housing', 'plate-steel', 'plate-composite', 'plate-reactive', 'seam', 'glow', 'glow-enemy']
    .forEach(k => { COL[k] = root.getPropertyValue('--' + k).trim(); });
  const dpr = Math.min(window.devicePixelRatio || 1, 3);
  const $ = id => document.getElementById(id);
  const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

  // ---------- drawing primitives ----------
  function pathPts(ctx, pts) { ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]); ctx.closePath(); }
  function rrect(ctx, x, y, w, h, r) {
    ctx.beginPath(); ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
  }
  function glowDot(ctx, x, y, r, col, a, blur) {
    ctx.save(); ctx.globalAlpha = a; ctx.fillStyle = col; ctx.shadowColor = col; ctx.shadowBlur = blur;
    ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill(); ctx.restore();
  }
  function partPx(p, s, ox, oy) { return polygon(p).map(([x, y]) => [ox + x * s, oy + y * s]); }

  // Ship style 2a: flat tiles, thin dark seams, no bevels. Machines are one flat colour with their glowing icon on the tile.
  const ICONS = window.__shipIcons || {};
  // draw fn() as a pure blur (no crisp copy): the shape goes off-canvas and only its shadow lands here
  function softly(ctx, blur, col, fn) {
    ctx.save(); const m = ctx.getTransform(), K = 6000;
    ctx.translate(-K, 0); ctx.shadowColor = col; ctx.shadowBlur = blur * m.a; ctx.shadowOffsetX = K * m.a; ctx.shadowOffsetY = 0;
    fn(); ctx.restore();
  }
  function drawBase(ctx, p, s, ox, oy, a) {
    const d = PARTS[p.type], pts = partPx(p, s, ox, oy);
    ctx.save(); ctx.globalAlpha = a;
    pathPts(ctx, pts); ctx.fillStyle = d.hull ? COL['plate-' + (p.mat || 'steel')] : COL.housing; ctx.fill();
    ctx.strokeStyle = COL.seam; ctx.lineWidth = Math.max(1, s * 0.07); ctx.lineJoin = 'miter'; ctx.stroke();
    ctx.restore();
  }

  function drawArt(ctx, p, s, ox, oy, a, info) {
    info = info || {};
    const d = PARTS[p.type]; if (d.hull) return;
    const [w, h] = dims(p), bx = ox + p.x * s, by = oy + p.y * s, cx = bx + w * s / 2, cy = by + h * s / 2;
    ctx.save(); ctx.globalAlpha = a;
    if (p.type === 'beam' && info.clear !== false) { // barrel out through the nose, muzzle glow
      const bw = s * 0.25, top = by - s * 0.7, bot = by + s * 0.1;
      const g = ctx.createLinearGradient(cx - bw / 2, 0, cx + bw / 2, 0);
      [[0, '#2a3a40'], [0.45, '#a6c4c4'], [0.7, '#456064'], [1, '#1c272b']].forEach(([o, c]) => g.addColorStop(o, c));
      rrect(ctx, cx - bw / 2, top, bw, bot - top, s * 0.05); ctx.fillStyle = g; ctx.fill();
      ctx.strokeStyle = '#0a0f16'; ctx.lineWidth = Math.max(0.6, s * 0.03); ctx.stroke();
      softly(ctx, s * 0.16, 'rgba(92, 198, 214, 0.85)', () => { ctx.beginPath(); ctx.arc(cx, top, s * 0.15, 0, Math.PI * 2); ctx.fillStyle = COL.cyan; ctx.fill(); });
      ctx.fillStyle = '#eafcff'; ctx.beginPath(); ctx.arc(cx, top, s * 0.065, 0, Math.PI * 2); ctx.fill();
    }
    const img = ICONS[p.type];
    if (img && img.complete && img.naturalWidth) {
      ctx.translate(cx, cy); ctx.rotate((p.rot || 0) * Math.PI / 2); if (p.flip) ctx.scale(-1, 1);
      ctx.drawImage(img, -d.w * s / 2, -d.h * s / 2, d.w * s, d.h * s);
    }
    ctx.restore();
  }

  // teal plume with a dotted particle trail
  function flameLen(p, s, f) { return p.type === 'torch' ? s * (0.75 + 1.3 * f / 45) : s * (0.4 + 0.75 * f / 12); }
  function drawFlame(ctx, p, s, ox, oy, f, a, trail) {
    const big = p.type === 'torch', [w, h] = dims(p);
    const cx = ox + (p.x + w / 2) * s, y1 = oy + (p.y + h) * s - s * 0.05;
    const half = s * (big ? 0.75 : 0.3), len = flameLen(p, s, f);
    const tri = () => { ctx.beginPath(); ctx.moveTo(cx - half, y1); ctx.lineTo(cx + half, y1); ctx.lineTo(cx, y1 + len); ctx.closePath(); };
    ctx.save(); ctx.globalAlpha = a;
    softly(ctx, s * 0.32, 'rgba(140, 239, 219, 0.3)', () => { tri(); ctx.fillStyle = COL.glow; ctx.fill(); });
    const g = ctx.createLinearGradient(0, y1, 0, y1 + len);
    g.addColorStop(0, 'rgba(230, 255, 248, 0.9)'); g.addColorStop(0.25, 'rgba(140, 239, 219, 0.55)'); g.addColorStop(1, 'rgba(140, 239, 219, 0)');
    tri(); ctx.fillStyle = g; ctx.fill();
    if (trail !== false) {
      const n = big ? 7 : 6, gap = s * (big ? 0.62 : 0.37), r0 = s * (big ? 0.0975 : 0.039);
      ctx.fillStyle = COL.glow;
      for (let i = 1; i <= n; i++) {
        ctx.globalAlpha = a * 0.6 * (1 - i / 9.5);
        ctx.beginPath(); ctx.arc(cx, y1 + len + i * gap, Math.max(0.6, r0 * (1 - i / 12)), 0, Math.PI * 2); ctx.fill();
      }
    }
    ctx.restore();
  }

  // outer edges of a set of parts: diagonal slope faces always, straight sides unless a kept neighbour covers them
  function hullPath(ctx, parts, C, keep, s, ox, oy) {
    ctx.beginPath();
    parts.forEach((p, i) => {
      if (!keep(i)) return;
      const P = polygon(p);
      for (let k = 0; k < P.length; k++) {
        const [x1, y1] = P[k], [x2, y2] = P[(k + 1) % P.length];
        if (x1 !== x2 && y1 !== y2) { ctx.moveTo(ox + x1 * s, oy + y1 * s); ctx.lineTo(ox + x2 * s, oy + y2 * s); continue; }
        const n = Math.round(Math.abs(x2 - x1) + Math.abs(y2 - y1)), sx = Math.sign(x2 - x1), sy = Math.sign(y2 - y1);
        for (let t = 0; t < n; t++) {
          const ax = x1 + sx * t, ay = y1 + sy * t;
          const A = sy === 0 ? [Math.min(ax, ax + sx), ay - 1] : [ax - 1, Math.min(ay, ay + sy)];
          const B = sy === 0 ? [A[0], ay] : [ax, A[1]];
          const inA = occAt(C, A[0], A[1]) === i, out = inA ? B : A;
          const dOut = sy === 0 ? (inA ? 2 : 0) : (inA ? 1 : 3);
          const j = occAt(C, out[0], out[1]);
          if (j >= 0 && j !== i && keep(j) && C.edges[j].has(out[0] + ',' + out[1] + ',' + ((dOut + 2) % 4))) continue;
          ctx.moveTo(ox + ax * s, oy + ay * s); ctx.lineTo(ox + (ax + sx) * s, oy + (ay + sy) * s);
        }
      }
    });
  }
  function drawHullGlow(ctx, parts, C, keep, s, ox, oy, col) {
    ctx.save(); ctx.lineJoin = 'round'; ctx.lineCap = 'round';
    softly(ctx, s * 0.16, col, () => { ctx.globalAlpha = 0.22; hullPath(ctx, parts, C, keep, s, ox, oy); ctx.strokeStyle = col; ctx.lineWidth = s * 0.15; ctx.stroke(); });
    ctx.globalAlpha = 0.34; hullPath(ctx, parts, C, keep, s, ox, oy); ctx.strokeStyle = col; ctx.lineWidth = Math.max(1, s * 0.045); ctx.stroke();
    ctx.restore();
  }
  function drawAura(ctx, parts, s, ox, oy, col) {
    let x0 = W, x1 = 0, y0 = H, y1 = 0;
    parts.forEach(p => partCells(p).forEach(([x, y]) => { x0 = Math.min(x0, x); x1 = Math.max(x1, x + 1); y0 = Math.min(y0, y); y1 = Math.max(y1, y + 1); }));
    if (x1 <= x0) return;
    const cx = ox + (x0 + x1) / 2 * s, cy = oy + (y0 + y1) / 2 * s, R = Math.max(x1 - x0, y1 - y0) * s * 0.85;
    const cw = ctx.canvas.width / dpr, ch = ctx.canvas.height / dpr; // fade out before the canvas edge, never a visible box
    const rx = Math.max(s, Math.min(R, cx, cw - cx)), ry = Math.max(s, Math.min(R, cy, ch - cy));
    const rgb = col === COL['glow-enemy'] ? '236, 143, 124' : '140, 239, 219';
    const g = ctx.createRadialGradient(0, 0, 0, 0, 0, 1);
    g.addColorStop(0, 'rgba(' + rgb + ', 0.1)'); g.addColorStop(0.55, 'rgba(' + rgb + ', 0.04)'); g.addColorStop(1, 'rgba(' + rgb + ', 0)');
    ctx.save(); ctx.translate(cx, cy); ctx.scale(rx, ry); ctx.fillStyle = g; ctx.fillRect(-1, -1, 2, 2); ctx.restore();
  }
  function arrowHead(ctx, x, y, dx, dy, size, color) {
    const len = Math.hypot(dx, dy) || 1; dx /= len; dy /= len;
    ctx.beginPath(); ctx.moveTo(x, y);
    ctx.lineTo(x - dx * size - dy * size * 0.6, y - dy * size + dx * size * 0.6);
    ctx.lineTo(x - dx * size + dy * size * 0.6, y - dy * size - dx * size * 0.6);
    ctx.closePath(); ctx.fillStyle = color; ctx.fill();
  }
  function gunClear(C, p) {
    const [w] = dims(p);
    for (let k = 0; k < w; k++) for (let yy = p.y - 1; yy >= 0; yy--) if (C.occ[yy][p.x + k] >= 0) return false;
    return true;
  }

  function drawShip(ctx, parts, r, s, ox, oy, opt) {
    opt = opt || {};
    const isMain = i => r.main && r.partPiece[i] === r.main.id;
    const glow = opt.enemy ? COL['glow-enemy'] : COL.glow;
    if (opt.aura) drawAura(ctx, parts, s, ox, oy, glow);
    if (opt.flames !== false) r.s.engines.forEach(e => { if (e.f > 0) drawFlame(ctx, parts[e.i], s, ox, oy, e.f, 1); });
    parts.forEach((p, i) => drawBase(ctx, p, s, ox, oy, isMain(i) ? 1 : 0.45));
    parts.forEach((p, i) => {
      const d = PARTS[p.type]; if (d.hull) return;
      const pc = r.pieces[r.partPiece[i]], dark = (d.draw || 0) > 0 && pc.ratio === 0;
      drawArt(ctx, p, s, ox, oy, isMain(i) ? (dark ? 0.35 : 1) : 0.4, { clear: d.gun ? gunClear(r.C, p) : true });
    });
    if (r.main) drawHullGlow(ctx, parts, r.C, isMain, s, ox, oy, glow);
    const ov = opt.overlays; if (!ov) return;
    if (ov.heat) {
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const i = r.C.occ[y][x]; if (i < 0) continue;
        const d = PARTS[parts[i].type], kk = Math.min(1, r.T[y][x] / (d.limit || 3));
        ctx.save(); pathPts(ctx, partPx(parts[i], s, ox, oy)); ctx.clip();
        ctx.globalAlpha = 0.06 + kk * 0.5; ctx.fillStyle = kk > 0.6 ? COL.coral : COL.amber; ctx.fillRect(ox + x * s, oy + y * s, s, s);
        ctx.restore();
      }
    }
    if (ov.bonus) {
      r.links.forEach(l => {
        let pair = null;
        for (const [x, y] of partCells(parts[l.a])) { for (let d = 0; d < 4 && !pair; d++) { const nx = x + DIRS[d][0], ny = y + DIRS[d][1]; if (nx >= 0 && ny >= 0 && nx < W && ny < H && r.C.occ[ny][nx] === l.b && linked(r.C, x, y, d)) pair = [x, y, nx, ny]; } if (pair) break; }
        if (!pair) return;
        const col = l.kind === 'cap' ? COL.cyan : COL.amber;
        const ax = ox + (pair[0] + 0.5) * s, ay = oy + (pair[1] + 0.5) * s, bx = ox + (pair[2] + 0.5) * s, by = oy + (pair[3] + 0.5) * s;
        const mx = (ax + bx) / 2, my = (ay + by) / 2;
        ctx.save(); ctx.strokeStyle = col; ctx.lineWidth = Math.max(1.5, s / 18);
        ctx.beginPath(); ctx.moveTo(mx - (bx - ax) * 0.2, my - (by - ay) * 0.2); ctx.lineTo(mx + (bx - ax) * 0.2, my + (by - ay) * 0.2); ctx.stroke();
        ctx.fillStyle = COL.hull; ctx.beginPath(); ctx.arc(mx, my, s * 0.11, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
        ctx.lineWidth = Math.max(1, s / 30); ctx.beginPath();
        ctx.moveTo(mx - s * 0.05, my); ctx.lineTo(mx + s * 0.05, my); ctx.moveTo(mx, my - s * 0.05); ctx.lineTo(mx, my + s * 0.05); ctx.stroke();
        ctx.restore();
      });
    }
    r.s.hot.forEach(i => { ctx.save(); pathPts(ctx, partPx(parts[i], s, ox, oy)); ctx.strokeStyle = COL.coral; ctx.lineWidth = Math.max(2, s / 14); ctx.stroke(); ctx.restore(); });
    const cross = (x, y, sz) => { ctx.save(); ctx.strokeStyle = COL.coral; ctx.lineWidth = Math.max(2, s / 16); ctx.beginPath(); ctx.moveTo(x - sz, y - sz); ctx.lineTo(x + sz, y + sz); ctx.moveTo(x + sz, y - sz); ctx.lineTo(x - sz, y + sz); ctx.stroke(); ctx.restore(); };
    r.s.blocked.forEach(i => { const p = parts[i], [w] = dims(p); cross(ox + (p.x + w / 2) * s, oy + p.y * s + s * 0.14, s * 0.13); });
    r.s.engines.forEach(e => { if (e.open) return; const p = parts[e.i], [w, h] = dims(p); cross(ox + (p.x + w / 2) * s, oy + (p.y + h) * s, s * 0.16); });
    if (ov.mass && r.main && r.s.mass) {
      const S = r.s, cx = ox + S.cx * s, cy = oy + S.cy * s;
      if (S.thrust > 0) {
        const tx = ox + S.tx * s;
        ctx.save(); ctx.strokeStyle = COL.mint; ctx.globalAlpha = 0.75; ctx.setLineDash([4, 4]); ctx.lineWidth = 1.2;
        ctx.beginPath(); ctx.moveTo(tx, cy + s * 2.2); ctx.lineTo(tx, cy - s * 2.4); ctx.stroke(); ctx.setLineDash([]);
        ctx.globalAlpha = 0.9; arrowHead(ctx, tx, cy - s * 2.6, 0, -1, s * 0.22, COL.mint); ctx.restore();
      }
      ctx.save(); ctx.strokeStyle = COL.ink; ctx.lineWidth = 1.5;
      ctx.fillStyle = COL.hull; ctx.beginPath(); ctx.arc(cx, cy, s * 0.17, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(cx - s * 0.32, cy); ctx.lineTo(cx + s * 0.32, cy); ctx.moveTo(cx, cy - s * 0.32); ctx.lineTo(cx, cy + s * 0.32); ctx.stroke();
      ctx.fillStyle = COL.ink; ctx.beginPath(); ctx.arc(cx, cy, s * 0.17, -Math.PI / 2, 0); ctx.lineTo(cx, cy); ctx.fill();
      ctx.beginPath(); ctx.arc(cx, cy, s * 0.17, Math.PI / 2, Math.PI); ctx.lineTo(cx, cy); ctx.fill(); ctx.restore();
      const off = S.tx - S.cx;
      if (S.thrust > 0 && Math.abs(off) > 0.1) {
        const R = s * 0.75, cw = off < 0, a0 = -Math.PI * 0.78, a1 = -Math.PI * 0.22;
        ctx.save(); ctx.strokeStyle = COL.coral; ctx.lineWidth = Math.max(1.5, s / 18);
        ctx.beginPath(); if (cw) ctx.arc(cx, cy, R, a0, a1, false); else ctx.arc(cx, cy, R, a1, a0, true); ctx.stroke();
        const ae = cw ? a1 : a0, px = cx + Math.cos(ae) * R, py = cy + Math.sin(ae) * R;
        const dx = cw ? -Math.sin(ae) : Math.sin(ae), dy = cw ? Math.cos(ae) : -Math.cos(ae);
        arrowHead(ctx, px + dx * 2, py + dy * 2, dx, dy, s * 0.24, COL.coral); ctx.restore();
      }
    }
  }

  function partCanvas(p, s, pad) {
    const [w, h] = dims(p), d = PARTS[p.type];
    const showFlame = d.engine && pad > 2, f = (d.thrust || 0) * 0.6;
    const padTop = p.type === 'beam' && pad > 2 ? s * 0.85 : pad, padBottom = showFlame ? flameLen(p, s, f) + s * 0.1 : pad;
    const cw = w * s + 2 * pad, ch = h * s + padTop + padBottom;
    const cv = document.createElement('canvas');
    cv.width = Math.round(cw * dpr); cv.height = Math.round(ch * dpr); cv.style.width = cw + 'px'; cv.style.height = ch + 'px';
    const ctx = cv.getContext('2d'); ctx.scale(dpr, dpr);
    const q = Object.assign({}, p, { x: 0, y: 0 });
    if (showFlame) drawFlame(ctx, q, s, pad, padTop, f, 1, false);
    drawBase(ctx, q, s, pad, padTop, 1);
    drawArt(ctx, q, s, pad, padTop, 1, { clear: pad > 2 });
    return cv;
  }

  // ---------- state ----------
  const ORDER = ['block', 'slope1', 'slope2', 'slope3', 'fission', 'fusion', 'cap', 'pulse', 'beam', 'shield', 'rad', 'thruster', 'torch', 'core', 'cargo', 'hold'];
  const STORE = 'spacegame.drydock.v3';
  const clean = list => list.filter(p => p && PARTS[p.type]).map(p => ({ type: p.type, x: p.x | 0, y: p.y | 0, rot: (p.rot | 0) % 4, flip: !!p.flip, mat: MATS[p.mat] ? p.mat : 'steel' })).filter(fits);
  let parts = SAMPLE_MID();
  try { const saved = JSON.parse(localStorage.getItem(STORE) || 'null'); if (Array.isArray(saved)) parts = clean(saved); } catch (e) { /* keep sample */ }
  let tool = 'slope2', mat = 'composite', mirror = true;
  const ov = { heat: false, mass: true, bonus: true };
  let cell = 36, hover = null;
  function save() { try { localStorage.setItem(STORE, JSON.stringify(parts)); } catch (e) { } }

  // ---------- palette ----------
  const toolsEl = $('tools'), infoEl = $('toolInfo'), toolBtns = {};
  function buildTools() {
    toolsEl.innerHTML = '';
    let group = '';
    ORDER.concat(['X']).forEach(t => {
      const g = t === 'X' ? 'Other' : PARTS[t].group;
      if (g !== group) { group = g; const hd = document.createElement('span'); hd.className = 'tool-group'; hd.textContent = g; toolsEl.appendChild(hd); }
      const b = document.createElement('button');
      b.className = 'tool'; b.type = 'button'; b.id = 'tool-' + t;
      if (t === 'X') { b.style.setProperty('--tc', COL.faint); b.innerHTML = '<span class="x" aria-hidden="true">×</span>Remove'; }
      else {
        const d = PARTS[t];
        b.style.setProperty('--tc', COL[d.cat]);
        const box = document.createElement('span'); box.className = 'ic'; box.setAttribute('aria-hidden', 'true');
        const s = Math.floor(20 / Math.max(d.w, d.h));
        const cv = partCanvas({ type: t, x: 0, y: 0, rot: 0, flip: d.shape === 'slope', mat: d.hull ? mat : 'steel' }, s, 0);
        box.appendChild(cv); b.appendChild(box);
        const lab = document.createElement('span'); lab.textContent = d.short || d.name; b.appendChild(lab); b.title = d.name;
      }
      b.addEventListener('click', () => { tool = t; syncTools(); });
      toolsEl.appendChild(b); toolBtns[t] = b;
    });
    syncTools();
  }
  function syncTools() {
    Object.keys(toolBtns).forEach(k => toolBtns[k].setAttribute('aria-pressed', String(k === tool)));
    if (tool === 'X') infoEl.innerHTML = '<b>Remove</b>Click or drag to delete parts. Right-click also removes. Try cutting the sample ship in half.';
    else {
      const d = PARTS[tool];
      infoEl.innerHTML = '<b>' + d.name + (d.hull ? ' · ' + MATS[mat].name : '') + '</b>' + d.desc;
    }
  }
  document.querySelectorAll('[data-mat]').forEach(b => b.addEventListener('click', () => {
    mat = b.dataset.mat;
    document.querySelectorAll('[data-mat]').forEach(o => o.setAttribute('aria-pressed', String(o === b)));
    buildTools();
  }));

  // ---------- drydock ----------
  const canvas = $('dock'), ctx = canvas.getContext('2d'), wrap = $('wrap');
  function resize() {
    const w = wrap.clientWidth || 600;
    cell = Math.max(18, Math.min(42, Math.floor(w / W)));
    canvas.style.width = cell * W + 'px'; canvas.style.height = cell * H + 'px';
    canvas.width = Math.round(cell * W * dpr); canvas.height = Math.round(cell * H * dpr);
    render();
  }
  function candidate(cx, cy) {
    if (tool === 'X') return null;
    const d = PARTS[tool];
    const at = (rot, flip) => {
      const [w, h] = rot % 2 ? [d.h, d.w] : [d.w, d.h];
      return { type: tool, x: clamp(cx - Math.floor((w - 1) / 2), 0, W - w), y: clamp(cy - Math.floor((h - 1) / 2), 0, H - h), rot, flip, mat: d.hull ? mat : 'steel' };
    };
    if (d.shape !== 'slope') return at(0, false);
    const C = makeCtx(parts.filter(q => !partCells(q).some(([x, y]) => x === cx && y === cy)));
    let best = null, bestSc = -Infinity;
    for (let flip = 0; flip < 2; flip++) for (let rot = 0; rot < 4; rot++) {
      const p = at(rot, !!flip), sc = placementScoreAt(C, p);
      if (sc > bestSc) { bestSc = sc; best = p; }
    }
    return best;
  }
  function placementScoreAt(C, p) {
    // same scoring as the model's auto-orientation, applied to a given anchor
    if (!fits(p)) return -999;
    const cells = partCells(p), mine = new Set(cells.map(c => c.join(','))), fe = fullEdges(p);
    let sc = 0;
    for (const [x, y] of cells) {
      if (occAt(C, x, y) >= 0) sc -= 5;
      for (let d = 0; d < 4; d++) {
        const nx = x + DIRS[d][0], ny = y + DIRS[d][1];
        if (mine.has(nx + ',' + ny)) continue;
        const b = occAt(C, nx, ny), isFull = fe.has(x + ',' + y + ',' + d);
        if (b >= 0 && isFull && C.edges[b].has(nx + ',' + ny + ',' + ((d + 2) % 4))) sc += 2;
        else if (b >= 0 && !isFull) sc -= 1;
      }
    }
    const n = slopeNormal(p), [w] = dims(p), mid = p.x + w / 2, side = mid < W / 2 - 0.01 ? -1 : mid > W / 2 + 0.01 ? 1 : 0;
    return sc + 0.1 * n[0] * side - 0.05 * n[1];
  }
  const key = p => p.type + '|' + p.x + '|' + p.y + '|' + p.rot + '|' + (p.flip ? 1 : 0);
  const cellsKey = c => c[0] + ',' + c[1];
  function removeAt(list, cells) { const set = new Set(cells.map(cellsKey)); return list.filter(q => !partCells(q).some(c => set.has(cellsKey(c)))); }
  function put(p) {
    parts = removeAt(parts, partCells(p)); parts.push(p);
    if (mirror) {
      const m = mirrorPart(p), own = new Set(partCells(p).map(cellsKey));
      if (!partCells(m).some(c => own.has(cellsKey(c)))) { parts = removeAt(parts, partCells(m)); parts.push(m); }
    }
  }
  function partIndexAt(x, y) { return makeCtx(parts).occ[y][x]; }
  function act(cx, cy, isDown) {
    if (erasing || tool === 'X') {
      const cells = [[cx, cy]]; if (mirror) cells.push([W - 1 - cx, cy]);
      parts = removeAt(parts, cells); return;
    }
    const d = PARTS[tool], idx = partIndexAt(cx, cy), cur = idx >= 0 ? parts[idx] : null;
    if (cur && cur.type === tool) {
      if (d.hull && cur.mat !== mat) { // repaint material, keep shape
        const m = mirror ? parts.findIndex(q => key(q) === key(mirrorPart(cur))) : -1;
        cur.mat = mat; if (m >= 0) parts[m].mat = mat; return;
      }
      if (d.shape === 'slope' && isDown) { // turn the slope: next orientation that fits
        const m = mirror ? parts.findIndex(q => key(q) === key(mirrorPart(cur))) : -1;
        const others = parts.filter((q, i) => i !== idx && i !== m), C = makeCtx(others);
        let k = cur.rot + (cur.flip ? 4 : 0);
        for (let step = 1; step <= 8; step++) {
          const kk = (k + step) % 8, q = { type: cur.type, x: cur.x, y: cur.y, rot: kk % 4, flip: kk >= 4, mat: cur.mat };
          if (fits(q) && !partCells(q).some(([x, y]) => C.occ[y][x] >= 0)) {
            parts = others; parts.push(q);
            if (m >= 0) { const mq = mirrorPart(q); if (!partCells(mq).some(([x, y]) => makeCtx(parts).occ[y][x] >= 0)) parts.push(mq); }
            return;
          }
        }
      }
      return;
    }
    if (!isDown && (d.w > 1 || d.h > 1)) return;
    const p = candidate(cx, cy); if (p) put(p);
  }

  function render() {
    const r = analyze(parts), s = cell;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W * s, H * s);
    ctx.fillStyle = COL.line;
    for (let y = 0; y <= H; y++) for (let x = 0; x <= W; x++) ctx.fillRect(x * s - 1, y * s - 1, 2, 2);
    if (mirror) { ctx.save(); ctx.strokeStyle = COL.line; ctx.setLineDash([3, 5]); ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(W * s / 2, 0); ctx.lineTo(W * s / 2, H * s); ctx.stroke(); ctx.restore(); }
    drawShip(ctx, parts, r, s, 0, 0, { overlays: ov });
    if (hover && !painting) {
      const p = tool === 'X' ? null : candidate(hover[0], hover[1]);
      ctx.save(); ctx.setLineDash([4, 3]); ctx.lineWidth = 1.5; ctx.strokeStyle = COL.ink; ctx.globalAlpha = 0.7;
      if (p) { pathPts(ctx, partPx(p, s, 0, 0)); ctx.stroke(); }
      else { ctx.strokeStyle = COL.coral; ctx.strokeRect(hover[0] * s + 2, hover[1] * s + 2, s - 4, s - 4); }
      ctx.restore();
    }
    updateReadout(r);
  }

  function setCls(el, cls) { el.classList.remove('ok', 'bad', 'meh'); if (cls) el.classList.add(cls); }
  function updateReadout(r) {
    const m = r.main, S = r.s, status = $('status');
    const ids = ['roPower', 'roHeat', 'roAccel', 'roYaw', 'roFire', 'roArmor', 'roLogic', 'roCargo'];
    if (!m) {
      ids.forEach(id => { $(id).textContent = '–'; setCls($(id)); });
      $('roAccelSub').textContent = ''; $('roFireSub').textContent = ''; $('mPower').style.width = '0'; $('mHeat').style.width = '0';
      status.className = 'status'; status.textContent = 'Empty grid. Pick a part and paint, or load a sample ship.'; status.title = '';
      return;
    }
    $('roPower').textContent = m.draw + ' / ' + m.out;
    setCls($('roPower'), m.draw > m.out ? 'bad' : (m.out > 0 && m.draw / m.out > 0.9 ? 'meh' : 'ok'));
    $('mPower').style.width = (m.out ? Math.min(100, (m.draw / m.out) * 100) : (m.draw ? 100 : 0)) + '%';
    const peak = Math.round(S.peak * 100);
    $('roHeat').textContent = peak + '%'; setCls($('roHeat'), S.peak > 1 ? 'bad' : S.peak > 0.85 ? 'meh' : 'ok');
    $('mHeat').style.width = Math.min(100, peak) + '%';
    const acc = S.mass ? S.thrust / S.mass : 0;
    $('roAccel').textContent = acc.toFixed(2); $('roAccelSub').textContent = 'thrust ' + Math.round(S.thrust) + ' · mass ' + Math.round(S.mass);
    setCls($('roAccel'), acc === 0 ? 'bad' : acc < 0.3 ? 'meh' : 'ok');
    const off = S.tx - S.cx;
    if (S.thrust === 0) { $('roYaw').textContent = '–'; setCls($('roYaw')); }
    else if (Math.abs(off) <= 0.1) { $('roYaw').textContent = 'Balanced'; setCls($('roYaw'), 'ok'); }
    else { $('roYaw').textContent = 'Pulls ' + (off < 0 ? 'right' : 'left'); setCls($('roYaw'), 'bad'); }
    $('roFire').textContent = S.firepower.toFixed(1) + '×';
    $('roFireSub').textContent = S.guns ? S.guns + ' gun' + (S.guns > 1 ? 's' : '') + ' · 1× = one pulse laser' : 'no guns';
    setCls($('roFire'), S.blocked.length ? 'meh' : null);
    $('roArmor').textContent = S.armor.toFixed(1);
    $('roLogic').textContent = S.slots ? S.slots + ' slots' : 'Manual'; setCls($('roLogic'), S.slots ? 'ok' : 'meh');
    const adrift = r.pieces.length - 1;
    $('roCargo').textContent = S.cargo ? S.cargo + ' units' : 'None'; setCls($('roCargo'), S.cargo ? null : 'meh');
    const msgs = [];
    if (m.draw > 0 && m.out === 0) msgs.push('No reactor: nothing on this ship has power.');
    else if (m.draw > m.out) msgs.push('Brownout: systems run at ' + Math.round(m.ratio * 100) + '%. Add a reactor.');
    if (S.hot.length) msgs.push(S.hot.length + (S.hot.length > 1 ? ' parts are' : ' part is') + ' overheating and shut down. Vent heat through an outer edge.');
    if (S.blocked.length) msgs.push(S.blocked.length + (S.blocked.length > 1 ? ' guns have' : ' gun has') + ' no clear line of fire.');
    const blockedE = S.engines.filter(e => !e.open).length;
    if (blockedE) msgs.push(blockedE + (blockedE > 1 ? ' engines are' : ' engine is') + ' blocked. Exhaust needs open space behind.');
    if (adrift) { const dark = r.pieces.filter(p => p !== m && p.draw > 0 && p.out === 0).length; msgs.push('Ship split: ' + adrift + (adrift > 1 ? ' pieces' : ' piece') + ' adrift' + (dark ? ', ' + dark + ' without power' : '') + '. That is salvage now.'); }
    if (S.thrust > 0 && Math.abs(off) > 0.1) msgs.push('Off-center thrust: the ship pulls ' + (off < 0 ? 'right' : 'left') + '.');
    if (!S.engines.length) msgs.push('No engines. Without them this is a station.');
    if (!S.slots) msgs.push('No logic core: manual flight only, no behaviors.');
    status.title = '';
    if (!msgs.length) { status.className = 'status'; status.textContent = 'Flight-ready. Try swapping the beams for pulse lasers, or cut the ship in half.'; }
    else {
      status.className = 'status warn'; status.textContent = msgs[0];
      if (msgs.length > 1) { const sp = document.createElement('span'); sp.className = 'more'; sp.textContent = ' (+' + (msgs.length - 1) + ' more)'; status.appendChild(sp); status.title = msgs.slice(1).join('\n'); }
    }
  }

  // ---------- input ----------
  let painting = false, erasing = false, lastKey = '';
  function cellAt(e) {
    const rc = canvas.getBoundingClientRect();
    const x = Math.floor((e.clientX - rc.left) / rc.width * W), y = Math.floor((e.clientY - rc.top) / rc.height * H);
    return x >= 0 && y >= 0 && x < W && y < H ? [x, y] : null;
  }
  function apply(e, isDown) {
    const c = cellAt(e); if (!c) return;
    const k = c.join(','); if (k === lastKey) return; lastKey = k;
    act(c[0], c[1], isDown); render(); save();
  }
  canvas.addEventListener('pointerdown', e => {
    e.preventDefault(); painting = true; erasing = e.button === 2; lastKey = '';
    try { canvas.setPointerCapture(e.pointerId); } catch (err) { }
    apply(e, true);
  });
  canvas.addEventListener('pointermove', e => {
    if (painting) { apply(e, false); return; }
    if (e.pointerType === 'mouse') { const c = cellAt(e); const k = c ? c.join(',') : ''; if (k !== (hover ? hover.join(',') : '')) { hover = c; render(); } }
  });
  canvas.addEventListener('pointerleave', () => { if (hover) { hover = null; render(); } });
  ['pointerup', 'pointercancel', 'lostpointercapture'].forEach(n => canvas.addEventListener(n, () => { painting = false; }));
  canvas.addEventListener('contextmenu', e => e.preventDefault());

  function toggle(id, fn) { const b = $(id); b.addEventListener('click', () => { const on = b.getAttribute('aria-pressed') !== 'true'; b.setAttribute('aria-pressed', String(on)); fn(on); render(); }); }
  toggle('ovHeat', on => { ov.heat = on; });
  toggle('ovMass', on => { ov.mass = on; });
  toggle('ovBonus', on => { ov.bonus = on; });
  toggle('mirror', on => { mirror = on; });
  $('bevel').addEventListener('click', () => { parts = bevel(parts); save(); render(); });
  $('loadMid').addEventListener('click', () => { parts = SAMPLE_MID(); save(); render(); });
  $('loadEarly').addEventListener('click', () => { parts = SAMPLE_EARLY(); save(); render(); });
  $('clear').addEventListener('click', () => { parts = []; save(); render(); });

  // ---------- parts sheet ----------
  const hullSheet = $('hullSheet');
  if (hullSheet) {
    ['steel', 'composite', 'reactive'].forEach(mt => {
      const row = document.createElement('div'); row.className = 'sheet-row';
      const name = document.createElement('span'); name.className = 'sheet-name'; name.textContent = MATS[mt].name; row.appendChild(name);
      ['block', 'slope1', 'slope2', 'slope3'].forEach(t => {
        const cv = partCanvas({ type: t, x: 0, y: 0, rot: 0, flip: PARTS[t].shape === 'slope', mat: mt }, 30, 3);
        cv.setAttribute('role', 'img'); cv.setAttribute('aria-label', MATS[mt].name + ' ' + PARTS[t].name);
        row.appendChild(cv);
      });
      hullSheet.appendChild(row);
    });
  }
  const machSheet = $('machineSheet');
  if (machSheet) {
    ['fission', 'fusion', 'cap', 'pulse', 'beam', 'shield', 'rad', 'thruster', 'torch', 'core'].forEach(t => {
      const d = PARTS[t], f = document.createElement('figure');
      const cv = partCanvas({ type: t, x: 0, y: 0, rot: 0, flip: false, mat: 'steel' }, 30, 4);
      cv.setAttribute('role', 'img'); cv.setAttribute('aria-label', d.name);
      const cap = document.createElement('figcaption'); cap.innerHTML = '<b style="color:' + COL[d.cat] + '">' + d.name + '</b>' + d.w + '×' + d.h;
      f.appendChild(cv); f.appendChild(cap); machSheet.appendChild(f);
    });
  }

  // ---------- early vs mid ----------
  function drawCompare(id, list) {
    const cv = $(id); if (!cv) return;
    const box = cv.parentElement.clientWidth - 24 || 300;
    let minX = W, maxX = 0, minY = H, maxY = 0;
    list.forEach(p => partCells(p).forEach(([x, y]) => { minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y); }));
    const cols = 15, rows = 6;
    const s = Math.max(12, Math.min(26, Math.floor(box / (cols + 1))));
    const cw = (cols + 1) * s, ch = (rows + 3.2) * s;
    cv.width = Math.round(cw * dpr); cv.height = Math.round(ch * dpr); cv.style.width = cw + 'px'; cv.style.height = ch + 'px';
    const c2 = cv.getContext('2d'); c2.setTransform(dpr, 0, 0, dpr, 0, 0); c2.clearRect(0, 0, cw, ch);
    const ox = (cw - (maxX - minX + 1) * s) / 2 - minX * s, oy = s * 1.0 - minY * s;
    drawShip(c2, list, analyze(list), s, ox, oy, { aura: true });
  }
  function drawCompares() { drawCompare('shipEarly', SAMPLE_EARLY()); drawCompare('shipMid', SAMPLE_MID()); }

  // ---------- tech tree ----------
  const TREE = [
    { id: 'pulse', label: 'Pulse laser', kind: 'part', col: 0, row: 2 },
    { id: 'rapid', label: 'Rapid pulse', kind: 'variant', col: 1, row: 0, from: ['pulse'] },
    { id: 'heavy', label: 'Heavy pulse', kind: 'variant', col: 1, row: 1, from: ['pulse'] },
    { id: 'beam', label: 'Beam laser 1×2', kind: 'part', col: 1, row: 3, from: ['pulse'] },
    { id: 'optics', label: 'Focusing optics', kind: 'refine', col: 1, row: 4, from: ['pulse'] },
    { id: 'pd', label: 'Point-defense laser', kind: 'variant', col: 2, row: 0, from: ['rapid'] },
    { id: 'turret', label: 'Laser turret 2×2', kind: 'part', col: 2, row: 1, from: ['heavy'] },
    { id: 'focused', label: 'Focused beam', kind: 'variant', col: 2, row: 3, from: ['beam'] },
    { id: 'jackets', label: 'Cooling jackets', kind: 'refine', col: 2, row: 4, from: ['optics'], needs: 'Thermal' },
    { id: 'burst', label: 'Burst emitter', kind: 'variant', col: 3, row: 0, from: ['pd'] },
    { id: 'twin', label: 'Twin turret', kind: 'variant', col: 3, row: 1, from: ['turret'] },
    { id: 'lance', label: 'Spinal lance 1×4', kind: 'size', col: 3, row: 2, from: ['focused'] },
    { id: 'prism', label: 'Prism array', kind: 'alien', col: 3, row: 3, from: ['focused'], needs: 'Lattice' },
    { id: 'overcharge', label: 'Overcharge', kind: 'refine', col: 3, row: 4, from: ['jackets'], needs: 'Power' },
    { id: 'capital', label: 'Capital beam 3×3', kind: 'size', col: 4, row: 1, from: ['twin', 'lance'] },
    { id: 'phase', label: 'Phase lance', kind: 'alien', col: 4, row: 2, from: ['lance'], needs: 'Phase walkers' },
    { id: 'refract', label: 'Refraction grid', kind: 'alien', col: 4, row: 3, from: ['prism'], needs: 'Lattice' },
  ];
  function buildTree() {
    const host = $('tree'); if (!host) return;
    const NS = 'http://www.w3.org/2000/svg', cw = 196, rh = 70, nw = 164, nh = 44, padX = 8, padY = 30;
    const Wt = 5 * cw + padX, Ht = 5 * rh + padY + 6;
    const svg = document.createElementNS(NS, 'svg');
    svg.setAttribute('viewBox', '0 0 ' + Wt + ' ' + Ht); svg.setAttribute('width', Wt); svg.setAttribute('height', Ht);
    svg.setAttribute('role', 'img'); svg.setAttribute('aria-label', 'Energy weapons tech tree, 17 nodes from Pulse laser to Phase lance');
    const el = (n, attrs, parent) => { const e = document.createElementNS(NS, n); Object.keys(attrs).forEach(k => e.setAttribute(k, attrs[k])); (parent || svg).appendChild(e); return e; };
    const pos = {}; TREE.forEach(n => { pos[n.id] = [padX + n.col * cw, padY + n.row * rh]; });
    ['Start', 'Early', 'Mid', 'Late', 'Endgame'].forEach((t, i) => { const tx = el('text', { x: padX + i * cw, y: 14, fill: COL.faint, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 11, 'letter-spacing': '0.08em' }); tx.textContent = t.toUpperCase(); });
    TREE.forEach(n => (n.from || []).forEach(f => {
      const [x1, y1] = pos[f], [x2, y2] = pos[n.id];
      const ax = x1 + nw, ay = y1 + nh / 2, bx = x2, by = y2 + nh / 2, mx = (ax + bx) / 2;
      el('path', { d: `M${ax} ${ay} C ${mx} ${ay}, ${mx} ${by}, ${bx} ${by}`, fill: 'none', stroke: COL.line, 'stroke-width': 1.5 });
    }));
    TREE.forEach(n => {
      const [x, y] = pos[n.id], g = el('g', {});
      const alien = n.kind === 'alien', col = alien ? COL.violet : n.kind === 'refine' ? COL.steel : COL.cyan;
      const r = el('rect', { x: x + 0.5, y: y + 0.5, width: nw, height: nh, rx: 4, fill: n.kind === 'part' || alien ? col : COL.hull, 'fill-opacity': n.kind === 'part' || alien ? 0.13 : 1, stroke: col, 'stroke-width': 1.4 }, g);
      if (n.kind === 'refine') r.setAttribute('stroke-dasharray', '4 3');
      if (n.kind === 'size') el('rect', { x: x + 3.5, y: y + 3.5, width: nw - 6, height: nh - 6, rx: 2.5, fill: 'none', stroke: col, 'stroke-width': 1, 'stroke-opacity': 0.7 }, g);
      const t = el('text', { x: x + 10, y: y + (n.kind === 'refine' || n.needs ? 19 : 27), fill: COL.ink, 'font-family': 'Barlow, system-ui, sans-serif', 'font-size': 14, 'font-weight': 500 }, g); t.textContent = n.label;
      if (n.kind === 'refine') { for (let i = 0; i < 5; i++) el('rect', { x: x + 10 + i * 9, y: y + 28, width: 6, height: 6, fill: i < 2 ? col : 'none', stroke: col, 'stroke-width': 1 }, g); }
      if (n.needs) { const tx = el('text', { x: x + (n.kind === 'refine' ? 60 : 10), y: y + 35, fill: alien ? COL.violet : COL.amber, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 10.5 }, g); tx.textContent = (alien ? 'from ' : 'needs ') + n.needs; }
    });
    host.appendChild(svg);
  }

  // ---------- script tabs ----------
  document.querySelectorAll('[role="tab"]').forEach(tab => tab.addEventListener('click', () => {
    const list = tab.closest('[role="tablist"]');
    list.querySelectorAll('[role="tab"]').forEach(t => { const on = t === tab; t.setAttribute('aria-selected', String(on)); t.tabIndex = on ? 0 : -1; $(t.getAttribute('aria-controls')).hidden = !on; });
  }));


  // ---------- evolution lab (toy) ----------
  const evoShip = $('evoShip');
  if (evoShip) {
    const GENS = 80, POP = 30;
    let E = null, running = false, seed = 20261010;
    const CAT = { block: 'Armor', slope1: 'Armor', slope2: 'Armor', slope3: 'Armor', pulse: 'Weapons', beam: 'Weapons', cap: 'Power', fission: 'Power', fusion: 'Power', shield: 'Shields', rad: 'Thermal', thruster: 'Propulsion', torch: 'Propulsion', core: 'Logic', cargo: 'Cargo', hold: 'Cargo' };
    function drawEvoShip(list) {
      const box = evoShip.parentElement.clientWidth - 24 || 300;
      const s = Math.max(12, Math.min(24, Math.floor(box / 16)));
      const cw = 16 * s, ch = 9.5 * s;
      evoShip.width = Math.round(cw * dpr); evoShip.height = Math.round(ch * dpr); evoShip.style.width = cw + 'px'; evoShip.style.height = ch + 'px';
      const c2 = evoShip.getContext('2d'); c2.setTransform(dpr, 0, 0, dpr, 0, 0); c2.clearRect(0, 0, cw, ch);
      if (!list.length) return;
      let minY = H, maxY = 0; list.forEach(p => partCells(p).forEach(([, y]) => { minY = Math.min(minY, y); maxY = Math.max(maxY, y); }));
      drawShip(c2, list, analyze(list), s, 0, Math.max(s * 0.6, (ch - (maxY - minY + 1) * s) / 2 - s * 0.4) - minY * s, { aura: true });
    }
    function chart(hist) {
      const host = $('evoChart'), NS = 'http://www.w3.org/2000/svg';
      const Wc = 420, Hc = 170, L = 36, Rr = 44, T = 12, B = 26;
      const maxY = Math.max(100, Math.ceil(Math.max(0, ...hist) / 20) * 20);
      const sx = g => L + (g / GENS) * (Wc - L - Rr), sy = v => T + (1 - v / maxY) * (Hc - T - B);
      host.innerHTML = '';
      const svg = document.createElementNS(NS, 'svg');
      svg.setAttribute('viewBox', `0 0 ${Wc} ${Hc}`); svg.setAttribute('role', 'img');
      svg.setAttribute('aria-label', hist.length ? `Best score rose from ${hist[0].toFixed(0)} to ${hist[hist.length - 1].toFixed(0)} over ${hist.length - 1} generations` : 'Best score per generation, not run yet');
      const el = (n, a) => { const e = document.createElementNS(NS, n); Object.keys(a).forEach(k => e.setAttribute(k, a[k])); svg.appendChild(e); return e; };
      for (let k = 0; k <= 4; k++) {
        const v = maxY * k / 4, y = sy(v);
        el('line', { x1: L, x2: Wc - Rr, y1: y, y2: y, stroke: COL['line-soft'], 'stroke-width': 1 });
        const t = el('text', { x: L - 6, y: y + 3.5, 'text-anchor': 'end', fill: COL.faint, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 10 }); t.textContent = v.toFixed(0);
      }
      [0, 20, 40, 60, 80].forEach(g => { const t = el('text', { x: sx(g), y: Hc - 8, 'text-anchor': 'middle', fill: COL.faint, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 10 }); t.textContent = g; });
      const xl = el('text', { x: Wc - Rr, y: Hc - 8, 'text-anchor': 'start', dx: 8, fill: COL.faint, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 10 }); xl.textContent = 'gen';
      if (hist.length) {
        el('path', { d: hist.map((v, g) => (g ? 'L' : 'M') + sx(g).toFixed(1) + ' ' + sy(v).toFixed(1)).join(' '), fill: 'none', stroke: COL['chart-line'] || '#3fa070', 'stroke-width': 2, 'stroke-linejoin': 'round', 'stroke-linecap': 'round' });
        const g = hist.length - 1, v = hist[g];
        el('circle', { cx: sx(g), cy: sy(v), r: 4, fill: COL['chart-line'] || '#3fa070', stroke: COL.hull, 'stroke-width': 2 });
        const lab = el('text', { x: sx(g) + 8, y: sy(v) + 4, fill: COL.ink, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 11 }); lab.textContent = v.toFixed(0);
        // hover crosshair + tooltip
        const cross = el('line', { y1: T, y2: Hc - B, stroke: COL.faint, 'stroke-width': 1, visibility: 'hidden' });
        const dot = el('circle', { r: 4, fill: COL['chart-line'] || '#3fa070', stroke: COL.hull, 'stroke-width': 2, visibility: 'hidden' });
        const hit = el('rect', { x: L, y: T, width: Wc - L - Rr, height: Hc - T - B, fill: 'transparent', tabindex: 0 });
        const tip = $('evoTip');
        const show = gi => {
          gi = Math.max(0, Math.min(hist.length - 1, gi));
          cross.setAttribute('x1', sx(gi)); cross.setAttribute('x2', sx(gi)); cross.setAttribute('visibility', 'visible');
          dot.setAttribute('cx', sx(gi)); dot.setAttribute('cy', sy(hist[gi])); dot.setAttribute('visibility', 'visible');
          tip.hidden = false; tip.textContent = '';
          const b = document.createElement('b'); b.textContent = hist[gi].toFixed(1); tip.appendChild(b);
          tip.appendChild(document.createTextNode(' best score · generation ' + gi));
          const rc = svg.getBoundingClientRect(), hostRc = host.getBoundingClientRect();
          tip.style.left = Math.min(hostRc.width - 170, Math.max(0, (sx(gi) / Wc) * rc.width - 60)) + 'px';
        };
        let gi = hist.length - 1;
        hit.addEventListener('pointermove', e => { const rc = svg.getBoundingClientRect(); const x = (e.clientX - rc.left) / rc.width * Wc; gi = Math.round((x - L) / (Wc - L - Rr) * GENS); show(gi); });
        hit.addEventListener('pointerleave', () => { cross.setAttribute('visibility', 'hidden'); dot.setAttribute('visibility', 'hidden'); tip.hidden = true; });
        hit.addEventListener('focus', () => show(gi));
        hit.addEventListener('blur', () => { tip.hidden = true; cross.setAttribute('visibility', 'hidden'); dot.setAttribute('visibility', 'hidden'); });
        hit.addEventListener('keydown', e => { if (e.key === 'ArrowLeft') { gi = Math.max(0, gi - 1); show(gi); } if (e.key === 'ArrowRight') { gi = Math.min(hist.length - 1, gi + 1); show(gi); } });
      }
      host.appendChild(svg);
      const tbl = $('evoTable');
      if (tbl) {
        const rows = hist.map((v, g) => ({ g, v })).filter(r => r.g % 5 === 0 || r.g === hist.length - 1);
        tbl.innerHTML = '';
        const t = document.createElement('table'); t.className = 'evo-tbl';
        const hd = document.createElement('tr'); ['Generation', 'Best score'].forEach(h => { const th = document.createElement('th'); th.textContent = h; hd.appendChild(th); }); t.appendChild(hd);
        rows.forEach(r => { const tr = document.createElement('tr'); [r.g, r.v.toFixed(1)].forEach(c => { const td = document.createElement('td'); td.textContent = c; tr.appendChild(td); }); t.appendChild(tr); });
        tbl.appendChild(t);
      }
    }
    function describe(list) {
      const sc = toyScore(list), r = sc.r || analyze(list), S = r.s;
      $('evoGen').textContent = E ? E.gen + ' / ' + GENS : '0';
      $('evoScore').textContent = sc.score.toFixed(1);
      $('evoCost').textContent = shipCost(list).toFixed(1) + ' / ' + BUDGET;
      $('evoFire').textContent = (sc.fire || 0).toFixed(1) + '×';
      $('evoTough').textContent = (sc.tough || 0).toFixed(1);
      $('evoAgil').textContent = (sc.agility || 0).toFixed(2);
      const split = {}; list.forEach(p => { const k = CAT[p.type]; split[k] = (split[k] || 0) + cost(p); });
      const tot = shipCost(list) || 1;
      $('evoSplit').textContent = 'Points: ' + Object.keys(split).sort((a, b) => split[b] - split[a]).map(k => k.toLowerCase() + ' ' + Math.round(split[k] / tot * 100) + '%').join(' · ');
      const notes = [], count = t => list.filter(p => p.type === t).length;
      const lowest = Math.max(...list.map(p => p.y + dims(p)[1] - 1));
      const forward = S.engines.filter(e => { const p = list[e.i]; return p.y + dims(p)[1] - 1 < lowest; }).length;
      if (forward) notes.push(forward + ' of ' + S.engines.length + ' engines sit ahead of the stern. The rules only ask for open space behind an engine, so this is legal. A gap in the rules, or a feature?');
      if (!count('rad')) notes.push('No radiators at all: hull edges vent enough heat at this size.');
      else if (S.peak > 0.85) notes.push('Runs hot: peak heat ' + Math.round(S.peak * 100) + '%. The score never punishes heat directly, only lost firepower.');
      if (count('beam') && !count('pulse')) notes.push('Beam lasers replaced every pulse laser.');
      if (count('pulse') && !count('beam')) notes.push('Sticks with pulse lasers; beams cost too much under this budget.');
      if (count('cap') > count('pulse') + count('beam')) notes.push('More capacitors than guns: bonus stacking at work.');
      if ((split.Armor || 0) / tot > 0.35) notes.push('Spends ' + Math.round(split.Armor / tot * 100) + '% of its points on armor.');
      if (!notes.length) notes.push('Nothing unusual in this design.');
      const ul = $('evoNotes'); ul.innerHTML = ''; notes.slice(0, 3).forEach(n => { const li = document.createElement('li'); li.textContent = n; ul.appendChild(li); });
    }
    function show() { const list = expand(E.best ? E.best.g : E.pop[0].g); drawEvoShip(list); describe(list); chart(E.history); $('evoCaption').textContent = 'Best design · generation ' + E.gen + ' · seed ' + seed; }
    function run() {
      if (running) return;
      running = true; $('evoRun').disabled = true;
      E = evoInit(seed, POP);
      E.best = E.pop.reduce((a, b) => (b.f > a.f ? b : a)); E.history = [E.best.f];
      show();
      const tick = () => {
        evoStep(E); show();
        if (E.gen < GENS) setTimeout(tick, 0);
        else { running = false; $('evoRun').disabled = false; }
      };
      setTimeout(tick, 30);
    }
    E = evoInit(seed, POP); E.best = E.pop[0]; E.history = [];
    show(); $('evoCaption').textContent = 'Seed design · press Run or scroll here to start';
    $('evoRun').addEventListener('click', () => { if (E && E.gen > 0) seed = (seed * 1103515245 + 12345) >>> 0; run(); });
    $('evoOpen').addEventListener('click', () => { if (!E || !E.best) return; parts = expand(E.best.g).map(p => Object.assign({}, p)); save(); render(); $('drydock').scrollIntoView({ behavior: 'smooth' }); });
    if ('IntersectionObserver' in window) {
      const io = new IntersectionObserver(es => { if (es.some(e => e.isIntersecting)) { io.disconnect(); run(); } }, { threshold: 0.3 });
      io.observe(evoShip);
    }
  }


  // ---------- galaxy generator (sketch) ----------
  const galaxyHost = $('galaxy');
  if (galaxyHost) {
    const NS = 'http://www.w3.org/2000/svg', SZ = 720, CX = 360, CY = 360;
    const BANDS = [
      { id: 'core', name: 'Core', r0: 0, r1: 58 },
      { id: 'inner', name: 'Inner reach', r0: 58, r1: 142 },
      { id: 'middle', name: 'Middle reach', r0: 142, r1: 222 },
      { id: 'outer', name: 'Outer reach', r0: 222, r1: 292 },
      { id: 'rim', name: 'Rim', r0: 292, r1: 340 },
    ];
    const RACE = {
      automatons: { name: 'Rogue automatons', col: COL.steel },
      grown: { name: 'The Grown', col: COL.mint },
      swarm: { name: 'The Swarm', col: COL.cyan },
      lattice: { name: 'The Lattice', col: COL.shield },
      phase: { name: 'Phase walkers', col: COL.violet },
      precursor: { name: 'Precursors', col: COL.amber },
    };
    const SYL1 = ['Ka', 'Ve', 'Or', 'Th', 'Ny', 'Sel', 'Mar', 'Ix', 'Tal', 'Ry', 'Ost', 'Quen', 'Hal', 'Dra', 'Pel', 'Zor', 'Ae', 'Cor', 'Vel', 'Ush'];
    const SYL2 = ['ssa', 'rin', 'lon', 'dra', 'mae', 'tis', 'gor', 'vex', 'nar', 'cyr', 'eth', 'ul', 'ora', 'kai', 'phos', 'ven'];
    const SUF = ['Drift', 'Shoal', 'Verge', 'Cluster', 'Expanse', 'Reach', 'Deep', 'Knot', 'Scatter', 'Hollow'];
    const bandOf = r => BANDS.find(b => r >= b.r0 && r < b.r1) || BANDS[4];
    const bandIdx = id => BANDS.findIndex(b => b.id === id);

    function generate(seed) {
      const R = makeRng(seed), nodes = [], used = new Set();
      const name = () => { for (let k = 0; k < 20; k++) { const n = SYL1[Math.floor(R() * SYL1.length)] + SYL2[Math.floor(R() * SYL2.length)] + ' ' + SUF[Math.floor(R() * SUF.length)]; if (!used.has(n)) { used.add(n); return n; } } return 'Unnamed'; };
      nodes.push({ x: CX, y: CY, r: 0, a: 0, band: 'core', name: 'The Archive', tags: ['archive'] });
      let tries = 0;
      while (nodes.length < 80 && tries < 9000) {
        tries++;
        const a = R() * Math.PI * 2, r = 64 + Math.sqrt(R()) * 270;
        const x = CX + Math.cos(a) * r, y = CY + Math.sin(a) * r;
        const minD = r > 290 ? 40 : 44;
        if (nodes.some(n => Math.hypot(n.x - x, n.y - y) < minD)) continue;
        nodes.push({ x, y, r, a, band: bandOf(r).id, name: name(), tags: [] });
      }
      nodes.forEach(n => { n.bi = bandIdx(n.band); });
      // lanes: minimum spanning tree plus a few short extra links
      const n = nodes.length, inTree = new Array(n).fill(false), best = new Array(n).fill(Infinity), par = new Array(n).fill(-1), lanes = [];
      const d = (i, j) => Math.hypot(nodes[i].x - nodes[j].x, nodes[i].y - nodes[j].y);
      best[0] = 0;
      for (let k = 0; k < n; k++) {
        let u = -1; for (let i = 0; i < n; i++) if (!inTree[i] && (u < 0 || best[i] < best[u])) u = i;
        inTree[u] = true; if (par[u] >= 0) lanes.push([par[u], u]);
        for (let v = 0; v < n; v++) if (!inTree[v] && d(u, v) < best[v]) { best[v] = d(u, v); par[v] = u; }
      }
      const has = (i, j) => lanes.some(([a, b]) => (a === i && b === j) || (a === j && b === i));
      for (let i = 1; i < n; i++) {
        const near = [...Array(n).keys()].filter(j => j !== i).sort((a, b) => d(i, a) - d(i, b)).slice(0, 3);
        near.forEach(j => { if (!has(i, j) && d(i, j) < 78 && R() < 0.35) lanes.push([i, j]); });
      }
      const adj = nodes.map(() => []); lanes.forEach(([a, b]) => { adj[a].push(b); adj[b].push(a); });
      // home on the rim
      const homeAng = R() * Math.PI * 2;
      const angDiff = (a, b) => Math.abs(Math.atan2(Math.sin(a - b), Math.cos(a - b)));
      const rim = nodes.map((_, i) => i).filter(i => nodes[i].band === 'rim');
      const home = rim.sort((a, b) => angDiff(nodes[a].a, homeAng) - angDiff(nodes[b].a, homeAng))[0];
      nodes[home].tags.push('home'); nodes[home].name = 'The Shipyard';
      // races per band; the inner reach is split between Lattice and Phase walkers
      const split = R() * Math.PI * 2;
      nodes.forEach(nd => {
        nd.race = { rim: 'automatons', outer: 'grown', middle: 'swarm', core: 'precursor' }[nd.band] || (Math.sin(nd.a - split) > 0 ? 'lattice' : 'phase');
      });
      // gates with guardians on each band boundary: 3 per boundary, spread around the disc
      const gatesFor = bi => {
        const band = BANDS[bi], cands = nodes.map((_, i) => i).filter(i => nodes[i].bi === bi && nodes[i].r < band.r0 + 34);
        const picked = [];
        const target = [homeAng, homeAng + 2.1, homeAng - 2.1];
        target.forEach(t => { const c = cands.filter(i => !picked.includes(i)).sort((a, b) => angDiff(nodes[a].a, t) - angDiff(nodes[b].a, t))[0]; if (c !== undefined) picked.push(c); });
        picked.forEach(i => nodes[i].tags.push('gate'));
        return picked;
      };
      const gates = [4, 3, 2, 1].map(gatesFor); // rim->outer, outer->middle, middle->inner, inner->core
      nodes[home].tags.push('gate');
      // set pieces
      const pickIn = (bandId, count, tag, filter) => {
        const c = nodes.map((_, i) => i).filter(i => nodes[i].band === bandId && !nodes[i].tags.includes('home') && !nodes[i].tags.includes('gate') && (!filter || filter(nodes[i])));
        for (let k = 0; k < count && c.length; k++) { const j = Math.floor(R() * c.length); nodes[c[j]].tags.push(tag); c.splice(j, 1); }
      };
      pickIn('rim', 4, 'derelict'); pickIn('outer', 3, 'derelict'); pickIn('middle', 2, 'derelict'); pickIn('inner', 1, 'derelict');
      pickIn('outer', 2, 'station'); pickIn('inner', 3, 'station');
      pickIn('middle', 2, 'hive'); pickIn('outer', 1, 'giant'); pickIn('inner', 1, 'giant');
      pickIn('rim', 1, 'mega'); pickIn('outer', 1, 'mega'); pickIn('middle', 1, 'mega'); pickIn('inner', 2, 'mega');
      nodes.forEach(nd => {
        if (nd.band === 'outer' && !nd.tags.length && R() < 0.35) nd.tags.push('heat');
        if (nd.band === 'middle' && !nd.tags.includes('gate') && R() < 0.4) nd.tags.push('ion');
      });
      // outposts next to the first two gates the player has opened
      [gates[0][0], gates[1][0]].forEach(g => { if (g === undefined) return; const nb = adj[g].filter(j => !nodes[j].tags.length && nodes[j].bi === nodes[g].bi); if (nb.length) nodes[nb[0]].tags.push('outpost'); });
      // frontier: the player has opened the rim and outer gates; middle reach is being explored
      const frontierGate = gates[1][0];
      nodes.forEach(nd => { nd.fog = nd.bi < 2; });
      // a sample short voyage: frontier gate -> two clusters -> back
      let route = null;
      if (frontierGate !== undefined) {
        const a1 = adj[frontierGate].filter(j => nodes[j].bi === 2)[0];
        if (a1 !== undefined) route = [frontierGate, a1];
      }
      return { nodes, lanes, home, gates, route, frontierGate };
    }

    let gseed = 4207, curG = null;
    const tip = $('galaxyTip');
    function describeNode(nd) {
      const bits = [], t = nd.tags;
      if (t.includes('home')) bits.push('Home: your shipyard and drydock. Research happens here.');
      if (t.includes('archive')) bits.push('Precursor archive, guarded by the final guardian.');
      if (t.includes('gate') && !t.includes('home')) bits.push('Precursor gate, guarded. Beat the guardian to add it to your network.');
      if (t.includes('outpost')) bits.push('Outpost: refill supplies, heavy repairs.');
      if (t.includes('station')) bits.push(RACE[nd.race].name + ' station: standing, tech, missions.');
      if (t.includes('hive')) bits.push('Boss: swarm hive.');
      if (t.includes('giant')) bits.push('Boss: giant ' + RACE[nd.race].name.replace('The ', '') + ' ship.');
      if (t.includes('mega')) bits.push('Precursor megastructure.');
      if (t.includes('derelict')) bits.push('Derelict graveyard: salvage and lore.');
      if (t.includes('heat')) bits.push('Hazard: stellar heat.');
      if (t.includes('ion')) bits.push('Hazard: ion storm and nebula.');
      if (nd.bi <= 1) bits.push('Hazard: radiation.');
      if (!bits.length) bits.push('Ordinary cluster: patrols, asteroids, wrecks.');
      return bits;
    }
    function drawGalaxy() {
      const G = generate(gseed), svg = document.createElementNS(NS, 'svg'); curG = G;
      svg.setAttribute('viewBox', `0 0 ${SZ} ${SZ}`); svg.setAttribute('role', 'img');
      svg.setAttribute('aria-label', 'Generated galaxy map: ' + G.nodes.length + ' star clusters in five bands from the rim to the core, linked by lanes');
      const el = (n, a, parent) => { const e = document.createElementNS(NS, n); Object.keys(a).forEach(k => e.setAttribute(k, a[k])); (parent || svg).appendChild(e); return e; };
      const defs = el('defs', {});
      const rg = el('radialGradient', { id: 'rad' }, defs);
      el('stop', { offset: '0%', 'stop-color': COL.coral, 'stop-opacity': 0.22 }, rg);
      el('stop', { offset: '100%', 'stop-color': COL.coral, 'stop-opacity': 0 }, rg);
      el('circle', { cx: CX, cy: CY, r: 150, fill: 'url(#rad)' });
      BANDS.forEach(b => { if (b.r1 < 340) el('circle', { cx: CX, cy: CY, r: b.r1, fill: 'none', stroke: COL.line, 'stroke-width': 1, 'stroke-dasharray': '2 5' }); });
      BANDS.forEach(b => { if (b.id === 'core') return; const t = el('text', { x: CX, y: CY - b.r1 + 14, 'text-anchor': 'middle', fill: COL.faint, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 10, 'letter-spacing': '0.12em' }); t.textContent = b.name.toUpperCase(); });
      // territories
      const terr = el('g', {});
      G.nodes.forEach(nd => { if (nd.race === 'precursor') return; el('circle', { cx: nd.x, cy: nd.y, r: 30, fill: RACE[nd.race].col, 'fill-opacity': nd.fog ? 0.035 : 0.075 }, terr); });
      // lanes
      G.lanes.forEach(([a, b]) => { const A = G.nodes[a], B = G.nodes[b]; el('line', { x1: A.x, y1: A.y, x2: B.x, y2: B.y, stroke: COL.line, 'stroke-width': 1.3, 'stroke-opacity': A.fog || B.fog ? 0.45 : 1 }); });
      // sample voyage
      if (G.route) {
        const pts = G.route.map(i => G.nodes[i]);
        el('polyline', { points: pts.map(p => p.x.toFixed(1) + ',' + p.y.toFixed(1)).join(' '), fill: 'none', stroke: COL.mint, 'stroke-width': 2.2, 'stroke-dasharray': '5 4', 'stroke-linecap': 'round' });
      }
      // gate network: home to opened gates
      const opened = [G.gates[0][0], G.frontierGate].filter(i => i !== undefined);
      opened.forEach(i => { const A = G.nodes[G.home], B = G.nodes[i]; el('path', { d: `M${A.x} ${A.y} Q ${(A.x + B.x) / 2 + (CY - (A.y + B.y) / 2) * 0.15} ${(A.y + B.y) / 2 + ((A.x + B.x) / 2 - CX) * 0.15} ${B.x} ${B.y}`, fill: 'none', stroke: COL.amber, 'stroke-width': 1.2, 'stroke-opacity': 0.55, 'stroke-dasharray': '1 4', 'stroke-linecap': 'round' }); });
      // nodes
      G.nodes.forEach((nd, i) => {
        const g = el('g', { tabindex: 0, class: 'gnode' }), t = nd.tags, op = nd.fog ? 0.4 : 1;
        g.setAttribute('opacity', op);
        if (t.includes('heat')) el('circle', { cx: nd.x, cy: nd.y, r: 11, fill: 'none', stroke: COL.coral, 'stroke-width': 1.4, 'stroke-dasharray': '2 2.4' }, g);
        if (t.includes('ion')) el('circle', { cx: nd.x, cy: nd.y, r: 13, fill: COL.steel, 'fill-opacity': 0.08, stroke: COL.steel, 'stroke-width': 1, 'stroke-dasharray': '4 3' }, g);
        el('circle', { cx: nd.x, cy: nd.y, r: 16, fill: 'transparent' }, g); // hit area
        if (t.includes('archive')) { el('circle', { cx: nd.x, cy: nd.y, r: 12, fill: 'none', stroke: COL.amber, 'stroke-width': 2 }, g); el('circle', { cx: nd.x, cy: nd.y, r: 5, fill: COL.amber }, g); }
        else if (t.includes('home')) { el('rect', { x: nd.x - 7, y: nd.y - 7, width: 14, height: 14, fill: COL.hull, stroke: COL.violet, 'stroke-width': 2, transform: `rotate(45 ${nd.x} ${nd.y})` }, g); el('circle', { cx: nd.x, cy: nd.y, r: 2.5, fill: COL.violet }, g); }
        else if (t.includes('gate')) { el('circle', { cx: nd.x, cy: nd.y, r: 7, fill: COL.hull, stroke: COL.amber, 'stroke-width': 2 }, g); el('circle', { cx: nd.x, cy: nd.y, r: 2, fill: COL.amber }, g); const opn = opened.includes(i); el('path', { d: `M${nd.x - 4.5} ${nd.y - 10} l4.5 -7 l4.5 7 z`, fill: opn ? 'none' : COL.amber, stroke: COL.amber, 'stroke-width': 1.2 }, g); }
        else el('circle', { cx: nd.x, cy: nd.y, r: 3.6, fill: RACE[nd.race].col, 'fill-opacity': 0.9 }, g);
        if (t.includes('station')) el('rect', { x: nd.x + 6, y: nd.y - 12, width: 7, height: 7, fill: RACE[nd.race].col, stroke: COL.hull, 'stroke-width': 1 }, g);
        if (t.includes('outpost')) el('rect', { x: nd.x + 6, y: nd.y - 12, width: 7, height: 7, fill: COL.hull, stroke: COL.steel, 'stroke-width': 1.4 }, g);
        if (t.includes('hive')) { const r = 7.5, pts = [...Array(6).keys()].map(k => [nd.x + r * Math.cos(k * Math.PI / 3), nd.y + r * Math.sin(k * Math.PI / 3)].map(v => v.toFixed(1)).join(',')).join(' '); el('polygon', { points: pts, fill: 'none', stroke: COL.coral, 'stroke-width': 1.8 }, g); }
        if (t.includes('giant')) el('rect', { x: nd.x - 6, y: nd.y - 6, width: 12, height: 12, fill: 'none', stroke: COL.coral, 'stroke-width': 1.8, transform: `rotate(45 ${nd.x} ${nd.y})` }, g);
        if (t.includes('mega')) el('circle', { cx: nd.x, cy: nd.y, r: 8, fill: 'none', stroke: COL.amber, 'stroke-width': 1.2, 'stroke-dasharray': '3 2' }, g);
        if (t.includes('derelict')) el('path', { d: `M${nd.x + 5} ${nd.y + 5} l6 6 M${nd.x + 11} ${nd.y + 5} l-6 6`, stroke: COL.steel, 'stroke-width': 1.5 }, g);
        const showTip = () => {
          const rc = svg.getBoundingClientRect(), hr = galaxyHost.getBoundingClientRect();
          tip.textContent = '';
          const b = document.createElement('b'); b.textContent = nd.name; tip.appendChild(b);
          const sub = document.createElement('span'); sub.className = 'gt-sub'; sub.textContent = BANDS[nd.bi].name + ' · ' + RACE[nd.race].name + (nd.fog ? ' · unexplored' : ''); tip.appendChild(sub);
          describeNode(nd).forEach(line => { const s2 = document.createElement('span'); s2.textContent = line; tip.appendChild(s2); });
          tip.hidden = false;
          const px = (nd.x / SZ) * rc.width + (rc.left - hr.left), py = (nd.y / SZ) * rc.height + (rc.top - hr.top);
          tip.style.left = Math.max(4, Math.min(hr.width - 230, px + 14)) + 'px';
          tip.style.top = Math.max(4, Math.min(hr.height - 120, py - 10)) + 'px';
        };
        g.addEventListener('pointerenter', showTip); g.addEventListener('focus', showTip);
        g.addEventListener('pointerleave', () => { tip.hidden = true; }); g.addEventListener('blur', () => { tip.hidden = true; });
        g.addEventListener('click', () => { showTip(); drawCluster(i); });
        g.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); drawCluster(i); } });
      });
      galaxyHost.querySelectorAll('svg').forEach(s => s.remove());
      galaxyHost.insertBefore(svg, tip);
      $('galaxySeed').textContent = 'seed ' + gseed + ' · ' + G.nodes.length + ' clusters';
      drawCluster(G.route ? G.route[1] : G.home);
    }
    // ---------- cluster close-up ----------
    function drawCluster(idx) {
      const G = curG, nd = G.nodes[idx], R = makeRng(gseed * 131 + idx * 7919 + 1), t = nd.tags;
      const host = $('clusterView'); if (!host) return;
      const W2 = 520, H2 = 340, cx = W2 / 2, cy = H2 / 2;
      const svg = document.createElementNS(NS, 'svg');
      svg.setAttribute('viewBox', `0 0 ${W2} ${H2}`); svg.setAttribute('role', 'img');
      const el = (n, a, parent) => { const e = document.createElementNS(NS, n); Object.keys(a).forEach(k => e.setAttribute(k, a[k])); (parent || svg).appendChild(e); return e; };
      const label = (x, y, txt, col, anchor) => { const e = el('text', { x, y, fill: col || COL.dim, 'font-family': 'IBM Plex Mono, monospace', 'font-size': 10.5, 'text-anchor': anchor || 'middle' }); e.textContent = txt; return e; };
      const used = [];
      const spot = (r0, r1, minD) => { for (let k = 0; k < 200; k++) { const a = R() * Math.PI * 2, r = r0 + R() * (r1 - r0), x = cx + Math.cos(a) * r * 1.45, y = cy + Math.sin(a) * r * 0.85; if (x < 40 || x > W2 - 40 || y < 30 || y > H2 - 34) continue; if (used.every(u => Math.hypot(u[0] - x, u[1] - y) > (minD || 66))) { used.push([x, y]); return [x, y]; } } return [cx + 140, cy]; };
      const defs = el('defs', {});
      const heat = t.includes('heat'), rad = nd.bi <= 1;
      const sg = el('radialGradient', { id: 'starGlow' }, defs);
      el('stop', { offset: '0%', 'stop-color': heat ? COL.coral : COL.amber, 'stop-opacity': 0.55 }, sg);
      el('stop', { offset: '100%', 'stop-color': heat ? COL.coral : COL.amber, 'stop-opacity': 0 }, sg);
      if (rad) el('rect', { x: 0, y: 0, width: W2, height: H2, fill: COL.coral, 'fill-opacity': nd.bi === 0 ? 0.1 : 0.05 });
      el('circle', { cx, cy, r: heat ? 78 : 46, fill: 'url(#starGlow)' });
      el('circle', { cx, cy, r: t.includes('archive') ? 14 : 8, fill: heat ? COL.coral : COL.amber });
      used.push([cx, cy]);
      if (heat) { el('ellipse', { cx, cy, rx: 92, ry: 62, fill: 'none', stroke: COL.coral, 'stroke-width': 1.2, 'stroke-dasharray': '3 3' }); label(cx, cy - 68, 'heat zone', COL.coral); }
      // asteroid belt
      const belt = el('g', { fill: COL.steel, 'fill-opacity': 0.45 });
      const gap = R() * Math.PI * 2;
      for (let k = 0; k < 70; k++) {
        const a = R() * Math.PI * 2; if (Math.abs(Math.atan2(Math.sin(a - gap), Math.cos(a - gap))) < 0.5) continue;
        const r = 112 + R() * 20, x = cx + Math.cos(a) * r * 1.45, y = cy + Math.sin(a) * r * 0.85, s = 1.5 + R() * 3;
        const pts = [...Array(5).keys()].map(j => { const aa = j / 5 * Math.PI * 2, rr = s * (0.7 + R() * 0.5); return (x + Math.cos(aa) * rr).toFixed(1) + ',' + (y + Math.sin(aa) * rr).toFixed(1); }).join(' ');
        el('polygon', { points: pts }, belt);
      }
      // ion storm
      if (t.includes('ion')) { const [x, y] = spot(40, 95, 90); for (let k = 0; k < 4; k++) el('circle', { cx: x + (R() - 0.5) * 60, cy: y + (R() - 0.5) * 34, r: 22 + R() * 16, fill: COL.steel, 'fill-opacity': 0.06, stroke: COL.steel, 'stroke-width': 1, 'stroke-dasharray': '4 3' }); label(x, y + 4, 'ion storm', COL.dim); }
      // lane exits
      const nbs = G.lanes.filter(([a, b]) => a === idx || b === idx).map(([a, b]) => (a === idx ? b : a));
      nbs.forEach(j => {
        const o = G.nodes[j], ang = Math.atan2(o.y - nd.y, o.x - nd.x);
        const ex = cx + Math.cos(ang) * (W2 / 2 - 22), ey = cy + Math.sin(ang) * (H2 / 2 - 18);
        el('path', { d: `M${ex - 6} ${ey - 6} L${ex} ${ey} L${ex - 6} ${ey + 6}`, transform: `rotate(${ang * 180 / Math.PI} ${ex} ${ey})`, fill: 'none', stroke: COL.mint, 'stroke-width': 1.6 });
        label(Math.max(60, Math.min(W2 - 60, ex - Math.cos(ang) * 30)), Math.max(14, Math.min(H2 - 8, ey - Math.sin(ang) * 16 + 3)), 'lane: ' + o.name.split(' ')[0], COL.faint);
      });
      // set pieces
      const raceCol = RACE[nd.race].col;
      if (t.includes('home')) { const [x, y] = spot(30, 60); el('rect', { x: x - 12, y: y - 12, width: 24, height: 24, transform: `rotate(45 ${x} ${y})`, fill: COL.hull, stroke: COL.violet, 'stroke-width': 2 }); el('rect', { x: x - 26, y: y - 3, width: 52, height: 6, fill: 'none', stroke: COL.violet, 'stroke-width': 1 }); label(x, y + 26, 'shipyard · drydock · research', COL.violet); }
      if (t.includes('archive')) { [26, 40, 54].forEach((r, k) => el('circle', { cx, cy, r, fill: 'none', stroke: COL.amber, 'stroke-width': 1.4, 'stroke-dasharray': k === 1 ? '6 4' : 'none', 'stroke-opacity': 0.8 })); label(cx, cy + 72, 'the precursor archive', COL.amber); }
      if (t.includes('gate') && !t.includes('home')) { const [x, y] = spot(70, 105); el('circle', { cx: x, cy: y, r: 11, fill: COL.hull, stroke: COL.amber, 'stroke-width': 2.2 }); el('path', { d: `M${x - 6} ${y - 16} l6 -10 l6 10 z`, fill: COL.amber }); label(x, y + 24, 'gate + guardian', COL.amber); }
      if (t.includes('station')) { const [x, y] = spot(55, 100); el('rect', { x: x - 8, y: y - 8, width: 16, height: 16, fill: raceCol, 'fill-opacity': 0.8 }); el('circle', { cx: x, cy: y, r: 16, fill: 'none', stroke: raceCol, 'stroke-width': 1 }); label(x, y + 28, RACE[nd.race].name.replace('The ', '') + ' station · missions', raceCol); }
      if (t.includes('outpost')) { const [x, y] = spot(55, 100); el('rect', { x: x - 7, y: y - 7, width: 14, height: 14, fill: COL.hull, stroke: COL.steel, 'stroke-width': 1.8 }); label(x, y + 22, 'outpost', COL.steel); }
      if (t.includes('hive')) { const [x, y] = spot(60, 100, 70); const r = 15, pts = [...Array(6).keys()].map(k => (x + r * Math.cos(k * Math.PI / 3)).toFixed(1) + ',' + (y + r * Math.sin(k * Math.PI / 3)).toFixed(1)).join(' '); el('polygon', { points: pts, fill: COL.coral, 'fill-opacity': 0.15, stroke: COL.coral, 'stroke-width': 2 }); for (let k = 0; k < 14; k++) { const a = R() * Math.PI * 2, rr = 22 + R() * 22; el('circle', { cx: x + Math.cos(a) * rr, cy: y + Math.sin(a) * rr * 0.8, r: 1.6, fill: COL.cyan }); } label(x, y + 32, 'swarm hive (boss)', COL.coral); }
      if (t.includes('giant')) { const [x, y] = spot(40, 95, 70); el('ellipse', { cx: x, cy: y, rx: 52, ry: 26, fill: 'none', stroke: COL.coral, 'stroke-width': 1, 'stroke-dasharray': '5 4' }); el('rect', { x: x - 10, y: y - 10, width: 20, height: 20, transform: `rotate(45 ${x} ${y})`, fill: COL.coral, 'fill-opacity': 0.15, stroke: COL.coral, 'stroke-width': 2 }); label(x, y + 40, 'giant ship (roaming boss)', COL.coral); }
      if (t.includes('mega')) { const [x, y] = spot(50, 95, 80); for (let k = 0; k < 3; k++) { const a0 = k * 2.1 + R() * 0.3; el('path', { d: `M${x + Math.cos(a0) * 24} ${y + Math.sin(a0) * 24} A 24 24 0 0 1 ${x + Math.cos(a0 + 1.4) * 24} ${y + Math.sin(a0 + 1.4) * 24}`, fill: 'none', stroke: COL.amber, 'stroke-width': 3 }); } label(x, y + 38, 'megastructure', COL.amber); }
      const wrecks = t.includes('derelict') ? 7 : 2;
      const [wx, wy] = spot(45, 100);
      for (let k = 0; k < wrecks; k++) { const x = wx + (R() - 0.5) * (wrecks > 2 ? 70 : 34), y = wy + (R() - 0.5) * (wrecks > 2 ? 34 : 16); el('rect', { x: x - 4, y: y - 2.5, width: 8 + R() * 6, height: 5, transform: `rotate(${R() * 180} ${x} ${y})`, fill: COL.steel, 'fill-opacity': 0.55 }); }
      label(wx, wy + (wrecks > 2 ? 30 : 20), wrecks > 2 ? 'derelict graveyard' : 'wrecks', COL.steel);
      // patrols
      const pc = nd.race === 'precursor' ? COL.amber : raceCol;
      for (let k = 0; k < (nd.fog ? 1 : 2); k++) {
        const [x, y] = spot(35, 100, 60), rx = 30 + R() * 22, ry = 16 + R() * 10;
        el('ellipse', { cx: x, cy: y, rx, ry, fill: 'none', stroke: pc, 'stroke-width': 1, 'stroke-dasharray': '2 4', 'stroke-opacity': 0.8 });
        const a = R() * Math.PI * 2, sx = x + Math.cos(a) * rx, sy = y + Math.sin(a) * ry;
        el('path', { d: `M${sx} ${sy - 5} l4 8 l-8 0 z`, fill: pc });
      }
      label(28, H2 - 26, 'patrols', pc, 'start');
      // anomalies to scan
      for (let k = 0; k < 2; k++) { const [x, y] = spot(30, 110, 60); el('circle', { cx: x, cy: y, r: 6, fill: 'none', stroke: COL.amber, 'stroke-width': 1.2 }); el('circle', { cx: x, cy: y, r: 1.6, fill: COL.amber }); if (k === 0) label(x, y + 18, 'scan target', COL.amber); }
      // scale
      el('line', { x1: W2 - 128, x2: W2 - 28, y1: H2 - 14, y2: H2 - 14, stroke: COL.faint, 'stroke-width': 1 });
      label(W2 - 78, H2 - 20, '≈ 10–15 min across', COL.faint);
      host.innerHTML = ''; host.appendChild(svg);
      svg.setAttribute('aria-label', 'Close-up of ' + nd.name + ': ' + describeNode(nd).join(' '));
      $('clusterCap').textContent = nd.name + ' · ' + BANDS[nd.bi].name + ' · ' + RACE[nd.race].name + (nd.fog ? ' · unexplored' : '');
    }
    $('galaxyNew').addEventListener('click', () => { gseed = (gseed * 1103515245 + 12345) % 100000; drawGalaxy(); });
    drawGalaxy();
  }


  // ---------- materials and recipe explorer ----------
  const MATL = {
    ironore: { name: 'Iron-nickel ore', kind: 'raw', src: 'harvest', where: 'Metallic asteroids, everywhere' },
    silicates: { name: 'Silicates', kind: 'raw', src: 'harvest', where: 'Stony asteroids, everywhere' },
    carbon: { name: 'Carbon dust', kind: 'raw', src: 'harvest', where: 'Dark asteroids' },
    ice: { name: 'Water ice', kind: 'raw', src: 'harvest', where: 'Comets and ice rings' },
    copper: { name: 'Copper ore', kind: 'raw', src: 'harvest', where: 'Metallic asteroids' },
    rare: { name: 'Rare earths', kind: 'raw', src: 'harvest', where: 'Dense asteroids, uncommon' },
    volatiles: { name: 'Volatiles', kind: 'raw', src: 'harvest', where: 'Gas pockets and comet tails' },
    fissile: { name: 'Fissiles', kind: 'raw', src: 'harvest', where: 'Outer reach, near hot stars' },
    he3: { name: 'Helium-3', kind: 'raw', src: 'harvest', where: 'Skimmed from stars; needs heat shielding' },
    nebgas: { name: 'Ionized gas', kind: 'raw', src: 'harvest', where: 'Ion storms and nebulae, middle reach' },
    isotopes: { name: 'Hot isotopes', kind: 'raw', src: 'harvest', where: 'Radiation zones, inner reach' },
    tissue: { name: 'Living tissue', kind: 'raw', src: 'grown', where: 'Grown wrecks' },
    spores: { name: 'Spore pods', kind: 'raw', src: 'grown', where: 'Grown wrecks and gardens' },
    heartwood: { name: 'Heartwood', kind: 'raw', src: 'grown', where: 'Giant Grown ships, Grown missions' },
    chitin: { name: 'Chitin', kind: 'raw', src: 'swarm', where: 'Swarm drones' },
    resin: { name: 'Hive resin', kind: 'raw', src: 'swarm', where: 'Swarm hives' },
    nervecore: { name: 'Nerve cores', kind: 'raw', src: 'swarm', where: 'Hive bosses' },
    shards: { name: 'Crystal shards', kind: 'raw', src: 'lattice', where: 'Lattice wrecks, crystal fields' },
    resonant: { name: 'Resonant crystal', kind: 'raw', src: 'lattice', where: 'Lattice missions, deep crystal fields' },
    residue: { name: 'Phase residue', kind: 'raw', src: 'phase', where: 'Phase walker wrecks' },
    riftsilt: { name: 'Rift silt', kind: 'raw', src: 'phase', where: 'Phase rifts, giant phase ships' },
    palloy: { name: 'Precursor alloy', kind: 'raw', src: 'precursor', where: 'Automaton wrecks, guardians' },
    pmatter: { name: 'Programmable matter', kind: 'raw', src: 'precursor', where: 'Automaton cores, megastructures' },
    gateshard: { name: 'Gate shards', kind: 'raw', src: 'precursor', where: 'Guardians' },
    archivekey: { name: 'Archive keys', kind: 'raw', src: 'precursor', where: 'Megastructures, the core' },
    steel: { name: 'Steel plate', kind: 'refined', from: { ironore: 2 }, via: 'Smelter' },
    wiring: { name: 'Wiring', kind: 'refined', from: { copper: 1 }, via: 'Smelter' },
    ceramic: { name: 'Ceramic', kind: 'refined', from: { silicates: 2 }, via: 'Kiln' },
    cfiber: { name: 'Carbon fiber', kind: 'refined', from: { carbon: 2 }, via: 'Kiln' },
    composite: { name: 'Composite panel', kind: 'refined', from: { cfiber: 2, ceramic: 1, volatiles: 1 }, via: 'Kiln' },
    coolant: { name: 'Coolant', kind: 'refined', from: { ice: 2, volatiles: 1 }, via: 'Cryo plant' },
    circuits: { name: 'Circuits', kind: 'refined', from: { wiring: 2, silicates: 1, rare: 1 }, via: 'Electronics bench' },
    optics: { name: 'Optics', kind: 'refined', from: { silicates: 2, rare: 1 }, via: 'Electronics bench' },
    radhard: { name: 'Hardened circuits', kind: 'refined', from: { circuits: 1, isotopes: 1 }, via: 'Electronics bench' },
    magnets: { name: 'Magnet coils', kind: 'refined', from: { rare: 2, wiring: 2 }, via: 'Magnetics lab' },
    supercond: { name: 'Superconductor', kind: 'refined', from: { rare: 1, coolant: 1, nebgas: 1 }, via: 'Magnetics lab' },
    plasma: { name: 'Plasma cells', kind: 'refined', from: { nebgas: 2, magnets: 1 }, via: 'Magnetics lab' },
    fuelrods: { name: 'Fuel rods', kind: 'refined', from: { fissile: 2, steel: 1 }, via: 'Fuel works' },
    pellets: { name: 'Fusion pellets', kind: 'refined', from: { he3: 2, ice: 1 }, via: 'Fusion workshop' },
    charges: { name: 'Explosive charges', kind: 'refined', from: { volatiles: 2, carbon: 1 }, via: 'Munitions bay' },
    biopoly: { name: 'Biopolymer', kind: 'refined', from: { tissue: 2, ice: 1 }, via: 'Bio-vat' },
    biocat: { name: 'Bio-catalyst', kind: 'refined', from: { spores: 2, biopoly: 1 }, via: 'Bio-vat' },
    chitcomp: { name: 'Chitin composite', kind: 'refined', from: { chitin: 2, resin: 1 }, via: 'Hive press' },
    crystal: { name: 'Refined crystal', kind: 'refined', from: { shards: 3, ceramic: 1 }, via: 'Crystal annealer' },
    stabilizer: { name: 'Phase stabilizer', kind: 'refined', from: { residue: 2, supercond: 1 }, via: 'Phase chamber' },
    adaptive: { name: 'Adaptive alloy', kind: 'refined', from: { palloy: 2, pmatter: 1 }, via: 'Precursor forge' },
  };
  const RECIPES = [
    { part: 'Steel armor block', needs: { steel: 2 } },
    { part: 'Composite armor', needs: { composite: 2 } },
    { part: 'Reactive armor', needs: { steel: 2, charges: 1 } },
    { part: 'Crystal armor', race: 'lattice', needs: { crystal: 2, ceramic: 1 } },
    { part: 'Living hull', race: 'grown', needs: { biopoly: 2, heartwood: 1 } },
    { part: 'Fission reactor', needs: { fuelrods: 2, steel: 2, coolant: 1 } },
    { part: 'Fusion reactor 2×2', needs: { pellets: 2, magnets: 4, supercond: 2, steel: 4 } },
    { part: 'Bio-reactor', race: 'grown', needs: { biocat: 2, biopoly: 2 } },
    { part: 'Pulse laser', needs: { optics: 1, circuits: 1, steel: 1 } },
    { part: 'Beam laser 1×2', needs: { optics: 3, supercond: 1, coolant: 1, steel: 2 } },
    { part: 'Prism array', race: 'lattice', needs: { crystal: 2, optics: 2, resonant: 1 } },
    { part: 'Phase lance', race: 'phase', needs: { stabilizer: 2, optics: 2, adaptive: 1 } },
    { part: 'Railgun', needs: { magnets: 3, steel: 3, supercond: 1 } },
    { part: 'Missile pod', needs: { charges: 2, circuits: 1, steel: 1 } },
    { part: 'Shield', needs: { magnets: 2, circuits: 2, plasma: 1 } },
    { part: 'Radiator', needs: { coolant: 1, steel: 1, wiring: 1 } },
    { part: 'Thruster', needs: { steel: 2, ceramic: 1 } },
    { part: 'Torch drive 2×2', needs: { pellets: 2, magnets: 4, ceramic: 4, supercond: 2 } },
    { part: 'Blink coil', race: 'phase', needs: { riftsilt: 2, stabilizer: 1, magnets: 2 } },
    { part: 'Drone hangar', race: 'swarm', needs: { chitcomp: 2, nervecore: 1, circuits: 1 } },
    { part: 'Logic core', needs: { circuits: 3, pmatter: 1 } },
    { part: 'Hardened logic core', needs: { radhard: 3, pmatter: 1 } },
    { part: 'Cargo bay', needs: { steel: 2 } },
  ];
  const SRC = {
    harvest: { name: 'Harvested', col: COL.steel },
    grown: { name: 'The Grown', col: COL.mint },
    swarm: { name: 'The Swarm', col: COL.cyan },
    lattice: { name: 'The Lattice', col: COL.shield },
    phase: { name: 'Phase walkers', col: COL.violet },
    precursor: { name: 'Precursor', col: COL.amber },
  };
  const recipeHost = $('recipeTree');
  if (recipeHost) {
    const list = $('recipeList'); let current = 'Fusion reactor 2×2';
    function expandNeeds(needs, mult, rawTot, ups) {
      Object.keys(needs).forEach(k => {
        const m = MATL[k], q = needs[k] * mult;
        if (m.kind === 'raw') rawTot[k] = (rawTot[k] || 0) + q;
        else { ups.add(m.via); expandNeeds(m.from, q, rawTot, ups); }
      });
    }
    function row(k, q, depth) {
      const m = MATL[k], li = document.createElement('li');
      li.style.setProperty('--d', depth);
      const qty = document.createElement('span'); qty.className = 'rq'; qty.textContent = q + '×';
      const nm = document.createElement('span'); nm.className = 'rn'; nm.textContent = m.name;
      li.appendChild(qty); li.appendChild(nm);
      const tag = document.createElement('span');
      if (m.kind === 'raw') { tag.className = 'rsrc'; tag.style.setProperty('--c', SRC[m.src].col); tag.textContent = m.where; }
      else { tag.className = 'rvia'; tag.textContent = m.via; }
      li.appendChild(tag);
      return li;
    }
    function build(ul, needs, mult, depth) {
      Object.keys(needs).forEach(k => {
        const q = needs[k] * mult, m = MATL[k];
        ul.appendChild(row(k, q, depth));
        if (m.kind !== 'raw') build(ul, m.from, q, depth + 1);
      });
    }
    function show(name) {
      current = name;
      const r = RECIPES.find(x => x.part === name);
      list.querySelectorAll('button').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.part === name)));
      recipeHost.textContent = '';
      const head = document.createElement('div'); head.className = 'rhead';
      const h = document.createElement('b'); h.textContent = r.part; head.appendChild(h);
      if (r.race) { const rc = document.createElement('small'); rc.textContent = 'from ' + SRC[r.race].name; rc.style.color = SRC[r.race].col; head.appendChild(rc); }
      recipeHost.appendChild(head);
      const ul = document.createElement('ul'); ul.className = 'rtree'; build(ul, r.needs, 1, 0); recipeHost.appendChild(ul);
      const rawTot = {}, ups = new Set(); expandNeeds(r.needs, 1, rawTot, ups);
      const sum = document.createElement('div'); sum.className = 'rsum';
      const t1 = document.createElement('div'); const b1 = document.createElement('b'); b1.textContent = 'Raw materials in storage'; t1.appendChild(b1);
      const chips = document.createElement('div'); chips.className = 'rchips';
      Object.keys(rawTot).sort((a, b) => rawTot[b] - rawTot[a]).forEach(k => { const c = document.createElement('span'); c.className = 'rchip'; c.style.setProperty('--c', SRC[MATL[k].src].col); c.textContent = rawTot[k] + ' ' + MATL[k].name; chips.appendChild(c); });
      t1.appendChild(chips); sum.appendChild(t1);
      const t2 = document.createElement('div'); const b2 = document.createElement('b'); b2.textContent = 'Hub upgrades needed'; t2.appendChild(b2);
      const chips2 = document.createElement('div'); chips2.className = 'rchips';
      (ups.size ? [...ups] : ['none beyond the basics']).forEach(u => { const c = document.createElement('span'); c.className = 'rchip up'; c.textContent = u; chips2.appendChild(c); });
      t2.appendChild(chips2); sum.appendChild(t2);
      recipeHost.appendChild(sum);
    }
    RECIPES.forEach(r => {
      const b = document.createElement('button'); b.type = 'button'; b.className = 'chip'; b.dataset.part = r.part; b.textContent = r.part;
      if (r.race) b.style.setProperty('--cc', SRC[r.race].col);
      b.addEventListener('click', () => show(r.part)); list.appendChild(b);
    });
    show(current);
    // catalog
    const cat = $('matCatalog');
    if (cat) {
      const groups = [['Harvested in clusters', k => MATL[k].kind === 'raw' && MATL[k].src === 'harvest'], ['From the races', k => MATL[k].kind === 'raw' && ['grown', 'swarm', 'lattice', 'phase'].includes(MATL[k].src)], ['Precursor', k => MATL[k].kind === 'raw' && MATL[k].src === 'precursor'], ['Refined automatically', k => MATL[k].kind === 'refined']];
      groups.forEach(([title, f]) => {
        const keys = Object.keys(MATL).filter(f);
        const div = document.createElement('div'); div.className = 'matgroup';
        const h = document.createElement('h4'); h.textContent = title + ' · ' + keys.length; div.appendChild(h);
        const chips = document.createElement('div'); chips.className = 'rchips';
        keys.forEach(k => { const m = MATL[k], c = document.createElement('span'); c.className = 'rchip' + (m.kind === 'refined' ? ' ref' : ''); if (m.kind === 'raw') c.style.setProperty('--c', SRC[m.src].col); c.textContent = m.name; c.title = m.kind === 'raw' ? m.where : m.via + ': ' + Object.keys(m.from).map(x => m.from[x] + ' ' + MATL[x].name).join(', '); chips.appendChild(c); });
        div.appendChild(chips); cat.appendChild(div);
      });
      $('matCount').textContent = Object.keys(MATL).length + ' materials: ' + Object.keys(MATL).filter(k => MATL[k].kind === 'raw').length + ' raw, ' + Object.keys(MATL).filter(k => MATL[k].kind === 'refined').length + ' refined';
    }
  }


  // ---------- damage sandbox ----------
  const fightCv = $('fightCanvas');
  if (fightCv) {
    const WEAP = {
      laser: { name: 'Laser', dmg: 1.2, pen: 1, kind: 'energy', col: COL.cyan, note: 'Burns the first block it touches. Shields absorb it well.' },
      rail: { name: 'Railgun', dmg: 2.4, pen: 3, kind: 'kinetic', col: COL.ink, note: 'Punches through up to three blocks. Ricochets off slopes at shallow angles.' },
      missile: { name: 'Missile', dmg: 2.6, pen: 0, splash: 1.7, kind: 'explosive', col: COL.coral, note: 'Explodes on first contact, damaging everything nearby. Shields catch it outright.' },
    };
    const CHAIN = { fission: [2.2, 1.7], fusion: [4, 2.4], cap: [1, 1.3], torch: [2.5, 2] };
    let weapon = 'rail', T = [], hp = [], maxhp = [], R = null, pools = new Map(), shots = [], fx = [], tally = null, s = 22, ox = 0, oy = 0, anim = 0, lastT = 0;
    const fctx = fightCv.getContext('2d');
    const partHP = p => (PARTS[p.type].hull ? pv(p, 'hp') * 3 : PARTS[p.type].w * PARTS[p.type].h * 2);
    const center = p => { const [w, h] = dims(p); return [p.x + w / 2, p.y + h / 2]; };
    function inPoly(x, y, P) { let c = false; for (let i = 0, j = P.length - 1; i < P.length; j = i++) { if (((P[i][1] > y) !== (P[j][1] > y)) && (x < (P[j][0] - P[i][0]) * (y - P[i][1]) / (P[j][1] - P[i][1]) + P[i][0])) c = !c; } return c; }
    function load(list) {
      T = list.map(p => Object.assign({}, p)); hp = T.map(partHP); maxhp = hp.slice(); pools = new Map();
      T.forEach(p => { if (p.type === 'shield') pools.set(p, 4); });
      tally = { shots: 0, hits: 0, ricochet: 0, absorbed: 0, destroyed: 0, chain: 0 };
      shots = []; fx = []; rebuild(); layout(); draw();
    }
    function rebuild() { R = analyze(T, 150); }
    function bubbles() {
      return T.map((p, i) => ({ p, i })).filter(o => o.p.type === 'shield' && R.pieces[R.partPiece[o.i]].ratio > 0)
        .map(o => { const [x, y] = center(o.p); return { p: o.p, x, y, r: 2.6 }; });
    }
    function layout() {
      const wpx = fightCv.parentElement.clientWidth || 600, hpx = Math.round(Math.min(440, Math.max(300, wpx * 0.6)));
      let minX = W, maxX = 0, minY = H, maxY = 0;
      T.forEach(p => partCells(p).forEach(([x, y]) => { minX = Math.min(minX, x); maxX = Math.max(maxX, x + 1); minY = Math.min(minY, y); maxY = Math.max(maxY, y + 1); }));
      if (!T.length) { minX = 0; maxX = W; minY = 0; maxY = H; }
      s = Math.max(12, Math.min(30, Math.floor(Math.min(wpx / (maxX - minX + 12), hpx / (maxY - minY + 7)))));
      ox = wpx / 2 - ((minX + maxX) / 2) * s; oy = hpx / 2 - ((minY + maxY) / 2) * s;
      fightCv.width = Math.round(wpx * dpr); fightCv.height = Math.round(hpx * dpr); fightCv.style.width = wpx + 'px'; fightCv.style.height = hpx + 'px';
    }
    const destroyQ = [];
    function damage(i, d) { if (hp[i] <= 0) return; hp[i] -= d; if (hp[i] <= 0) destroyQ.push(i); }
    function explode(x, y, r, d) {
      T.forEach((p, i) => {
        let best = Infinity; partCells(p).forEach(([cx, cy]) => { best = Math.min(best, Math.hypot(cx + 0.5 - x, cy + 0.5 - y)); });
        if (best < r) damage(i, d * (1 - 0.5 * best / r));
      });
      fx.push({ type: 'boom', x, y, r, t: 0 });
    }
    function resolveDestruction() {
      let guard = 0;
      while (destroyQ.length && guard++ < 200) {
        const i = destroyQ.shift(), p = T[i];
        if (!p || p._dead) continue;
        p._dead = true; tally.destroyed++;
        const ch = CHAIN[p.type];
        if (ch) { const [x, y] = center(p); tally.chain++; explode(x, y, ch[1], ch[0]); }
      }
      const keep = T.map((p, i) => !p._dead);
      T = T.filter((p, i) => keep[i]); hp = hp.filter((v, i) => keep[i]); maxhp = maxhp.filter((v, i) => keep[i]);
      rebuild();
    }
    function fire(gx, gy, aim) {
      if (!T.length) return;
      const w = WEAP[weapon]; tally.shots++;
      let x, y, dx, dy;
      if (aim) { x = gx; y = gy; dx = aim[0]; dy = aim[1]; const L = Math.hypot(dx, dy) || 1; dx /= L; dy /= L; }
      else {
        const tx = R.main ? R.s.cx : W / 2, ty = R.main ? R.s.cy : H / 2;
        dx = gx - tx; dy = gy - ty; const L = Math.hypot(dx, dy) || 1; dx /= L; dy /= L;
        x = tx + dx * 16; y = ty + dy * 16; dx = -dx; dy = -dy;
      }
      const path = [[x, y]], step = 0.035, shields = bubbles(), crossed = new Set();
      shields.forEach(b => { if (Math.hypot(x - b.x, y - b.y) < b.r) crossed.add(b.p); });
      let dmg = w.dmg, pen = w.pen, last = -1, done = false;
      const C = R.C, polys = T.map(polygon);
      for (let k = 0; k < 40 / step && !done; k++) {
        x += dx * step; y += dy * step;
        for (const b of shields) {
          if (crossed.has(b.p) || Math.hypot(x - b.x, y - b.y) >= b.r) continue;
          crossed.add(b.p);
          const pool = pools.get(b.p) || 0; if (pool <= 0.05) continue;
          if (w.kind === 'energy') { const a = Math.min(pool, dmg); pools.set(b.p, pool - a); dmg -= a; if (dmg <= 0.05) { tally.absorbed++; fx.push({ type: 'shield', x, y, t: 0 }); path.push([x, y]); done = true; break; } }
          else if (w.kind === 'kinetic') { const a = Math.min(pool, dmg * 0.4); pools.set(b.p, pool - a); dmg -= a; fx.push({ type: 'shield', x, y, t: 0 }); }
          else { pools.set(b.p, Math.max(0, pool - dmg)); tally.absorbed++; fx.push({ type: 'shield', x, y, t: 0 }); path.push([x, y]); done = true; break; }
        }
        if (done) break;
        const cx = Math.floor(x), cy = Math.floor(y);
        if (cx < 0 || cy < 0 || cx >= W || cy >= H) { if (k * step > 30) break; continue; }
        const i = C.occ[cy][cx];
        if (i < 0 || !inPoly(x, y, polys[i])) { last = -1; continue; }
        if (i === last) continue;
        last = i;
        const p = T[i];
        if (w.kind === 'kinetic' && PARTS[p.type].shape === 'slope') {
          const n = slopeNormal(p), P = polys[i], mx = (P[0][0] + P[2][0]) / 2, my = (P[0][1] + P[2][1]) / 2;
          const px = x - dx * step, py = y - dy * step;
          const outside = (px - mx) * n[0] + (py - my) * n[1] > -0.02;
          const cosI = -(dx * n[0] + dy * n[1]);
          if (outside && cosI > 0 && cosI < 0.5) {
            damage(i, dmg * 0.2); tally.ricochet++; fx.push({ type: 'spark', x, y, t: 0 });
            const dot = dx * n[0] + dy * n[1]; dx -= 2 * dot * n[0]; dy -= 2 * dot * n[1];
            path.push([x, y]); x += dx * 0.12; y += dy * 0.12; dmg *= 0.7; continue;
          }
        }
        tally.hits++;
        if (w.kind === 'explosive') { path.push([x, y]); explode(x, y, w.splash, dmg); done = true; break; }
        damage(i, dmg);
        if (w.kind === 'energy') { path.push([x, y]); fx.push({ type: 'burn', x, y, t: 0 }); done = true; break; }
        fx.push({ type: 'spark', x, y, t: 0 });
        pen--; dmg *= 0.6;
        if (pen <= 0) { path.push([x, y]); done = true; break; }
      }
      if (!done) path.push([x, y]);
      shots.push({ path, col: w.col, t: 0, w: w.kind === 'kinetic' ? 1.6 : 2.2 });
      resolveDestruction();
      updateFightStats();
      kick();
    }
    function updateFightStats() {
      const parts = R.pieces.length;
      $('fightStats').textContent = 'Shots ' + tally.shots + ' · hits ' + tally.hits + ' · ricochets ' + tally.ricochet + ' · absorbed by shields ' + tally.absorbed + ' · parts destroyed ' + tally.destroyed + ' · chain explosions ' + tally.chain + ' · pieces ' + parts;
    }
    function draw() {
      const c = fctx, wpx = fightCv.width / dpr, hpx = fightCv.height / dpr;
      c.setTransform(dpr, 0, 0, dpr, 0, 0); c.clearRect(0, 0, wpx, hpx);
      if (T.length) drawShip(c, T, R, s, ox, oy, { aura: true });
      // damage tint and cracks
      T.forEach((p, i) => {
        const k = 1 - Math.max(0, hp[i]) / maxhp[i]; if (k <= 0.02) return;
        c.save(); pathPts(c, partPx(p, s, ox, oy)); c.clip();
        c.globalAlpha = 0.12 + k * 0.45; c.fillStyle = COL.coral; c.fill();
        if (k > 0.4) { const [x0, y0] = center(p); c.globalAlpha = 0.8; c.strokeStyle = COL.void; c.lineWidth = 1.2; c.beginPath(); c.moveTo(ox + (x0 - 0.35) * s, oy + (y0 - 0.2) * s); c.lineTo(ox + x0 * s, oy + (y0 + 0.05) * s); c.lineTo(ox + (x0 + 0.3) * s, oy + (y0 - 0.25) * s); c.moveTo(ox + x0 * s, oy + (y0 + 0.05) * s); c.lineTo(ox + (x0 + 0.1) * s, oy + (y0 + 0.35) * s); c.stroke(); }
        c.restore();
      });
      // shield bubbles
      bubbles().forEach(b => {
        const f = Math.max(0, (pools.get(b.p) || 0) / 4);
        c.save(); c.beginPath(); c.arc(ox + b.x * s, oy + b.y * s, b.r * s, 0, Math.PI * 2);
        c.globalAlpha = 0.04 + 0.06 * f; c.fillStyle = COL.shield; c.fill();
        c.globalAlpha = 0.15 + 0.45 * f; c.strokeStyle = COL.shield; c.lineWidth = Math.max(1.4, s * 0.08); c.lineCap = 'round'; c.setLineDash([0.1, Math.max(4, s * 0.22)]); c.stroke(); c.restore();
      });
      // shots
      shots.forEach(sh => {
        const a = Math.max(0, 1 - sh.t / 0.9); if (a <= 0) return;
        c.save(); c.globalAlpha = a; c.strokeStyle = sh.col; c.lineWidth = sh.w; c.lineCap = 'round'; c.shadowColor = sh.col; c.shadowBlur = 6;
        c.beginPath(); sh.path.forEach(([x, y], k) => { const X = ox + x * s, Y = oy + y * s; if (k) c.lineTo(X, Y); else c.moveTo(X, Y); }); c.stroke(); c.restore();
      });
      // effects
      fx.forEach(e => {
        const X = ox + e.x * s, Y = oy + e.y * s;
        c.save();
        if (e.type === 'boom') { const a = Math.max(0, 1 - e.t / 0.7); c.globalAlpha = a * 0.8; c.strokeStyle = COL.coral; c.lineWidth = 2; c.beginPath(); c.arc(X, Y, e.r * s * Math.min(1, 0.3 + e.t * 2), 0, Math.PI * 2); c.stroke(); c.globalAlpha = a * 0.25; c.fillStyle = COL.amber; c.fill(); }
        else { const a = Math.max(0, 1 - e.t / 0.5), col = e.type === 'shield' ? COL.shield : e.type === 'burn' ? COL.cyan : COL.amber; c.globalAlpha = a; c.fillStyle = col; c.shadowColor = col; c.shadowBlur = 10; c.beginPath(); c.arc(X, Y, 3 + e.t * 10, 0, Math.PI * 2); c.fill(); }
        c.restore();
      });
      if (dragStart && dragNow && Math.hypot(dragNow[0] - dragStart[0], dragNow[1] - dragStart[1]) > 12) {
        const ddx = dragNow[0] - dragStart[0], ddy = dragNow[1] - dragStart[1], L = Math.hypot(ddx, ddy);
        c.save(); c.strokeStyle = WEAP[weapon].col; c.globalAlpha = 0.7; c.setLineDash([5, 5]); c.lineWidth = 1.4;
        c.beginPath(); c.moveTo(dragStart[0], dragStart[1]); c.lineTo(dragStart[0] + ddx / L * 2000, dragStart[1] + ddy / L * 2000); c.stroke();
        c.setLineDash([]); c.globalAlpha = 1; c.fillStyle = WEAP[weapon].col; c.beginPath(); c.arc(dragStart[0], dragStart[1], 4, 0, Math.PI * 2); c.fill(); c.restore();
      }
      // hull map (the HUD widget)
      const ms = Math.max(4, Math.round(s / 4)), mw = 16 * ms, mh = 9 * ms, mx0 = wpx - mw - 12, my0 = 12;
      c.save(); c.globalAlpha = 0.9; c.fillStyle = COL.void; c.fillRect(mx0 - 6, my0 - 6, mw + 12, mh + 26); c.strokeStyle = COL.line; c.strokeRect(mx0 - 6, my0 - 6, mw + 12, mh + 26);
      c.globalAlpha = 1; c.fillStyle = COL.faint; c.font = '10px "IBM Plex Mono", monospace'; c.fillText('HULL MAP', mx0, my0 + mh + 14);
      let bx0 = W, bx1 = 0, by0 = H, by1 = 0; T.forEach(p => partCells(p).forEach(([x, y]) => { bx0 = Math.min(bx0, x); bx1 = Math.max(bx1, x); by0 = Math.min(by0, y); by1 = Math.max(by1, y); }));
      T.forEach((p, i) => {
        const f = Math.max(0, hp[i]) / maxhp[i], col = f > 0.66 ? COL.mint : f > 0.33 ? COL.amber : COL.coral;
        const dark = R.partPiece[i] !== (R.main ? R.main.id : -1);
        c.globalAlpha = dark ? 0.3 : 0.85; c.fillStyle = col;
        partCells(p).forEach(([x, y]) => { c.fillRect(mx0 + (x - bx0) * ms + (mw - (bx1 - bx0 + 1) * ms) / 2, my0 + (y - by0) * ms + (mh - (by1 - by0 + 1) * ms) / 2, ms - 1, ms - 1); });
      });
      c.restore();
    }
    function kick() { if (!anim) { lastT = performance.now(); anim = requestAnimationFrame(tick); } }
    function tick(now) {
      const dt = Math.min(0.05, (now - lastT) / 1000); lastT = now;
      shots.forEach(sh => { sh.t += dt; }); fx.forEach(e => { e.t += dt; });
      shots = shots.filter(sh => sh.t < 0.9); fx = fx.filter(e => e.t < 0.8);
      let charging = false;
      bubbles().forEach(b => { const v = pools.get(b.p) || 0; if (v < 4) { pools.set(b.p, Math.min(4, v + 0.6 * dt)); charging = true; } });
      draw();
      if (shots.length || fx.length || charging) anim = requestAnimationFrame(tick); else anim = 0;
    }
    let dragStart = null, dragNow = null;
    const toPx = e => { const rc = fightCv.getBoundingClientRect(); return [(e.clientX - rc.left) * (fightCv.width / dpr / rc.width), (e.clientY - rc.top) * (fightCv.height / dpr / rc.height)]; };
    fightCv.addEventListener('pointerdown', e => { dragStart = toPx(e); dragNow = dragStart; try { fightCv.setPointerCapture(e.pointerId); } catch (err) { } });
    fightCv.addEventListener('pointermove', e => { if (!dragStart) return; dragNow = toPx(e); draw(); });
    fightCv.addEventListener('pointerup', e => {
      if (!dragStart) return;
      const a = dragStart, b = toPx(e); dragStart = null; dragNow = null;
      if (Math.hypot(b[0] - a[0], b[1] - a[1]) > 12) fire((a[0] - ox) / s, (a[1] - oy) / s, [b[0] - a[0], b[1] - a[1]]);
      else fire((a[0] - ox) / s, (a[1] - oy) / s);
    });
    fightCv.addEventListener('pointercancel', () => { dragStart = null; dragNow = null; draw(); });
    document.querySelectorAll('[data-weapon]').forEach(b => b.addEventListener('click', () => {
      weapon = b.dataset.weapon;
      document.querySelectorAll('[data-weapon]').forEach(o => o.setAttribute('aria-pressed', String(o === b)));
      $('fightNote').textContent = WEAP[weapon].note;
    }));
    $('fightReset').addEventListener('click', () => load(T.length && fightSource === 'dock' ? parts : SAMPLE_MID()));
    let fightSource = 'sample';
    $('fightDock').addEventListener('click', () => { fightSource = 'dock'; load(parts); });
    $('fightSample').addEventListener('click', () => { fightSource = 'sample'; load(SAMPLE_MID()); });
    $('fightNote').textContent = WEAP[weapon].note;
    load(SAMPLE_MID());
    if ('ResizeObserver' in window) new ResizeObserver(() => { layout(); draw(); }).observe(fightCv.parentElement);
  }

  document.querySelectorAll('[data-mat]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.mat === mat)));
  buildTools(); buildTree();
  if ('ResizeObserver' in window) {
    new ResizeObserver(resize).observe(wrap);
    const cmp = $('shipEarly'); if (cmp) new ResizeObserver(drawCompares).observe(cmp.parentElement.parentElement);
  } else { window.addEventListener('resize', () => { resize(); drawCompares(); }); }
  resize(); drawCompares();
});
