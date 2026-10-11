// Node runner for the wasm builds (V8, same engine as Chrome). Loads every engine's wasm module from
// web/pkg/<build>/ and runs scenario x engine x repeat (interleaved like the native runner).
//
// Usage: node node/run-node.mjs [--engines a,b] [--scenarios a,b] [--repeats 3] [--warmup 30] [--ticks 900]
//                               [--out results/node.jsonl] [--tick-hashes] [--label node] [--rep-base 0]
import { readFileSync, appendFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import os from 'node:os';

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = join(here, '..', 'web', 'pkg');
const args = process.argv.slice(2);
const get = (f, d) => { const i = args.indexOf(f); return i >= 0 ? args[i + 1] : d; };
const repeats = +get('--repeats', 3);
const warmup = +get('--warmup', 30);
const ticks = +get('--ticks', 900);
const out = get('--out', null);
const tickHashes = args.includes('--tick-hashes');
const label = get('--label', 'node');
const repBase = +get('--rep-base', 0);

const modules = {}; // engine id -> module
for (const build of ['rapier', 'box2d', 'avian']) {
  const js = join(pkgRoot, build, 'bench.js');
  if (!existsSync(js)) { console.error(`(no wasm build for ${build}: ${js} missing)`); continue; }
  const m = await import(pathToFileURL(js).href);
  m.initSync({ module: readFileSync(join(pkgRoot, build, 'bench_bg.wasm')) });
  for (const e of m.engines().split(',')) modules[e] = m;
}
const anyMod = Object.values(modules)[0];
if (!anyMod) { console.error('no wasm builds found; run build-wasm.sh first'); process.exit(1); }
const allEngines = ['rapier', 'rapier-compound', 'box2d-rust', 'avian', 'avian-compound'].filter(e => modules[e]);
const engines = get('--engines', allEngines.join(',')).split(',').filter(e => modules[e]);
const scenarios = get('--scenarios', anyMod.scenarios()).split(',');
if (out) mkdirSync(dirname(out), { recursive: true });

const load = () => os.loadavg().map(x => x.toFixed(2)).join(' ');
console.error(`run-node: node ${process.version}, engines ${engines}, scenarios ${scenarios}, repeats ${repeats}, ticks ${ticks}`);
for (let rep = repBase; rep < repBase + repeats; rep++) {
  for (const s of scenarios) {
    for (const e of engines) {
      const loadBefore = load();
      const json = modules[e].run_scenario(e, s, warmup, ticks, tickHashes);
      const r = JSON.parse(json);
      if (r.error) { console.error(r.error); continue; }
      r.rep = rep; r.label = label; r.node = process.version; r.load_before = loadBefore; r.load_after = load();
      const f = (x) => x.toFixed(3).padStart(7);
      console.log(`rep ${rep} ${s.padEnd(16)} ${e.padEnd(16)} step med ${f(r.step.median)} p95 ${f(r.step.p95)} max ${f(r.step.max)} | total med ${f(r.total.median)} p95 ${f(r.total.p95)}` +
        (r.rays ? ` | rays med ${f(r.rays.median)} hits ${r.ray_hits}` : '') +
        (r.split ? ` | split mean ${f(r.split.mean)} max ${f(r.split.max)}` : '') +
        ` | bodies ${r.bodies_initial}->${r.bodies_final} | hash ${r.state_hash} | load ${r.load_after}`);
      if (out) appendFileSync(out, JSON.stringify(r) + '\n');
    }
  }
}
