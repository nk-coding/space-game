// Drydock rules from the concept board, as plain JS (no drawing). Reference for the sim crate, NOT a spec:
// numbers are placeholders. Grid 16x12, forward = up (y decreases). Covers: part footprints and rotation/flip,
// slope polygons and which sides connect, power pool per connected piece, heat diffusion with venting through
// open edges, adjacency bonuses, thrust/mass/centre of mass/yaw, guns needing a clear column, engines needing
// open cells behind, splitting into pieces, build points and the toy evolution operators.

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
