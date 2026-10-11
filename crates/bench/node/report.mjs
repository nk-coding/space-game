// Aggregates JSON-lines results into markdown tables (stdout) and writes web/reference-hashes.json.
// Usage: node node/report.mjs results/native-st.jsonl results/native-mt.jsonl results/node.jsonl ...
// Per (label, engine, scenario) group: the median over repeats of each per-run statistic.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const files = process.argv.slice(2).filter(f => existsSync(f));
const runs = files.flatMap(f => readFileSync(f, 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l)));

const ENGINES = ['rapier', 'rapier-compound', 'box2d-rust', 'avian', 'avian-compound', 'box2d-c'];
const SCEN = ['ships', 'ships-merged', 'debris', 'raycasts', 'breakage', 'breakage-merged', 'asteroids', 'combined', 'combined-merged'];
const median = (a) => { const s = [...a].sort((x, y) => x - y); const n = s.length; return n === 0 ? NaN : n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2; };
const f2 = (x) => (x === undefined || Number.isNaN(x)) ? '–' : x >= 100 ? x.toFixed(0) : x >= 10 ? x.toFixed(1) : x.toFixed(2);

const groups = new Map();
for (const r of runs) {
  const label = r.label || r.platform;
  const k = `${label}|${r.engine}|${r.scenario}`;
  if (!groups.has(k)) groups.set(k, []);
  groups.get(k).push(r);
}
const labels = [...new Set(runs.map(r => r.label || r.platform))];
const g = (label, e, s) => groups.get(`${label}|${e}|${s}`) || [];
const agg = (rs, fn) => median(rs.map(fn).filter(x => x !== undefined && x !== null));

const out = [];
const p = (s = '') => out.push(s);

const labelName = { st: 'Native (VM), 1 thread', mt: 'Native (VM), multi-thread', node: 'wasm in Node (V8), 1 thread', browser: 'Browser' };

// Load averages per label
for (const label of labels) {
  const rs = runs.filter(r => (r.label || r.platform) === label);
  const l1 = rs.map(r => parseFloat((r.load_before || '').split(' ')[0])).filter(x => !Number.isNaN(x));
  if (l1.length) p(`- ${labelName[label] || label}: ${rs.length} runs, 1-min load average during runs ${Math.min(...l1).toFixed(1)}–${Math.max(...l1).toFixed(1)} (median ${median(l1).toFixed(1)}), threads ${[...new Set(rs.map(r => r.threads))].join('/')}`);
}
p();

// Step time tables: rows scenario, cols engine, cell "median / p95".
for (const label of labels) {
  const engines = ENGINES.filter(e => SCEN.some(s => g(label, e, s).length));
  if (!engines.length) continue;
  p(`### Step time, ${labelName[label] || label} (ms per tick, median / p95)`);
  p();
  p(`| Scenario | ${engines.join(' | ')} |`);
  p(`|---|${engines.map(() => '---:').join('|')}|`);
  for (const s of SCEN) {
    const cells = engines.map(e => { const rs = g(label, e, s); return rs.length ? `${f2(agg(rs, r => r.step.median))} / ${f2(agg(rs, r => r.step.p95))}` : '–'; });
    if (cells.every(c => c === '–')) continue;
    p(`| ${s} | ${cells.join(' | ')} |`);
  }
  p();
}

// Total tick (split + step + rays) for the scenarios where it differs from step.
p('### Whole tick (split + step + raycasts), ms, median / p95 / max');
p();
const totalEngines = ENGINES.filter(e => runs.some(r => r.engine === e));
p(`| Platform | Scenario | ${totalEngines.join(' | ')} |`);
p(`|---|---|${totalEngines.map(() => '---:').join('|')}|`);
for (const label of labels) for (const s of ['raycasts', 'breakage', 'breakage-merged', 'combined', 'combined-merged']) {
  const cells = totalEngines.map(e => { const rs = g(label, e, s); return rs.length ? `${f2(agg(rs, r => r.total.median))} / ${f2(agg(rs, r => r.total.p95))} / ${f2(agg(rs, r => r.total.max))}` : '–'; });
  if (cells.every(c => c === '–')) continue;
  p(`| ${label} | ${s} | ${cells.join(' | ')} |`);
}
p();

p('### Raycasts: 5000 closest-hit rays per tick, ms per batch (median / p95), hits per run');
p();
p(`| Platform | Scenario | ${totalEngines.join(' | ')} |`);
p(`|---|---|${totalEngines.map(() => '---:').join('|')}|`);
for (const label of labels) for (const s of ['raycasts', 'combined', 'combined-merged']) {
  const cells = totalEngines.map(e => { const rs = g(label, e, s).filter(r => r.rays); return rs.length ? `${f2(agg(rs, r => r.rays.median))} / ${f2(agg(rs, r => r.rays.p95))} (${rs[0].ray_hits})` : '–'; });
  if (cells.every(c => c === '–')) continue;
  p(`| ${label} | ${s} | ${cells.join(' | ')} |`);
}
p();

p('### Breakage: cost of one split event, ms (mean / max over the 30 events)');
p();
p(`| Platform | Scenario | ${totalEngines.join(' | ')} |`);
p(`|---|---|${totalEngines.map(() => '---:').join('|')}|`);
for (const label of labels) for (const s of ['breakage', 'breakage-merged', 'combined', 'combined-merged']) {
  const cells = totalEngines.map(e => { const rs = g(label, e, s).filter(r => r.split); return rs.length ? `${f2(agg(rs, r => r.split.mean))} / ${f2(agg(rs, r => r.split.max))}` : '–'; });
  if (cells.every(c => c === '–')) continue;
  p(`| ${label} | ${s} | ${cells.join(' | ')} |`);
}
p();

p('### Setup: time to create all bodies and colliders, ms (median)');
p();
p(`| Platform | Scenario | ${totalEngines.join(' | ')} |`);
p(`|---|---|${totalEngines.map(() => '---:').join('|')}|`);
for (const label of labels) for (const s of ['ships', 'ships-merged', 'combined']) {
  const cells = totalEngines.map(e => { const rs = g(label, e, s); return rs.length ? f2(agg(rs, r => r.setup_ms)) : '–'; });
  if (cells.every(c => c === '–')) continue;
  p(`| ${label} | ${s} | ${cells.join(' | ')} |`);
}
p();

// Determinism
p('### Determinism (final running state hash over all ticks)');
p();
p('Columns: same hash on every repeat within the platform; native 1-thread == Node wasm; native multi-thread == native 1-thread.');
p();
p('| Engine | Scenario | native repeats | Node repeats | native == Node | MT == ST | native hash |');
p('|---|---|---|---|---|---|---|');
const reference = {};
for (const e of ENGINES) for (const s of SCEN) {
  const st = g('st', e, s), nd = g('node', e, s), mt = g('mt', e, s);
  if (!st.length && !nd.length) continue;
  const hs = (rs) => [...new Set(rs.map(r => r.state_hash))];
  const same = (rs) => rs.length ? (hs(rs).length === 1 ? `yes (${rs.length})` : `**no** (${hs(rs).length} distinct / ${rs.length})`) : '–';
  const eq = (a, b) => (a.length && b.length) ? (hs(a).length === 1 && hs(b).length === 1 && hs(a)[0] === hs(b)[0] ? 'yes' : '**no**') : '–';
  p(`| ${e} | ${s} | ${same(st)} | ${same(nd)} | ${eq(st, nd)} | ${eq(mt, st)} | \`${st.length ? hs(st)[0] : '–'}\` |`);
  const ticks = (st[0] || nd[0]).warmup + (st[0] || nd[0]).ticks;
  reference[`${e}/${s}/${ticks}`] = { native: st.length && hs(st).length === 1 ? hs(st)[0] : undefined, node: nd.length && hs(nd).length === 1 ? hs(nd)[0] : undefined };
}
p();

// box2d-rust (Rust port) vs box2d-c (C original): same hash means the port is bit-exact.
{
  const pairs = SCEN.map(s => [s, g('st', 'box2d-rust', s), g('st', 'box2d-c', s)]).filter(([, a, b]) => a.length && b.length);
  if (pairs.length) {
    const same = pairs.filter(([, a, b]) => a[0].state_hash === b[0].state_hash).map(([s]) => s);
    p(`box2d-rust vs box2d-c (native, 1 thread): identical state hash in ${same.length} of ${pairs.length} scenarios${same.length < pairs.length ? ' (differ: ' + pairs.filter(([s]) => !same.includes(s)).map(([s]) => s).join(', ') + ')' : ''}.`);
    const mt = SCEN.map(s => [s, g('mt', 'box2d-c', s), g('st', 'box2d-c', s)]).filter(([, a, b]) => a.length && b.length);
    if (mt.length) p(`box2d-c multi-thread vs 1 thread: identical in ${mt.filter(([, a, b]) => a[0].state_hash === b[0].state_hash).length} of ${mt.length} scenarios.`);
    p();
  }
}

// Ray query determinism
p('Raycast result hash (hit body + hit distance bits), native vs Node:');
p();
for (const e of ENGINES) for (const s of ['raycasts', 'combined']) {
  const st = g('st', e, s), nd = g('node', e, s);
  if (!st.length || !nd.length) continue;
  const a = [...new Set(st.map(r => r.ray_hash))], b = [...new Set(nd.map(r => r.ray_hash))];
  p(`- ${e} / ${s}: native ${a.join(',')} · Node ${b.join(',')} → ${a.length === 1 && b.length === 1 && a[0] === b[0] ? 'match' : 'differ'}`);
}
p();

writeFileSync(join(here, '..', 'web', 'reference-hashes.json'), JSON.stringify(reference, null, 1));
console.log(out.join('\n'));
