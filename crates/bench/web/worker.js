// Module worker: loads the per-engine wasm builds and runs jobs posted by the page, so the page stays
// responsive while a scenario runs.
const builds = ['rapier', 'box2d', 'avian'];
const modules = {}; // engine id -> module

async function load() {
  const info = [];
  for (const b of builds) {
    try {
      const m = await import(`./pkg/${b}/bench.js`);
      await m.default({ module_or_path: `./pkg/${b}/bench_bg.wasm` });
      for (const e of m.engines().split(',')) modules[e] = m;
      info.push({ build: b, engines: m.engines(), scenarios: m.scenarios() });
    } catch (err) {
      info.push({ build: b, error: String(err) });
    }
  }
  return info;
}

const ready = load();

self.onmessage = async (ev) => {
  const msg = ev.data;
  const info = await ready;
  if (msg.type === 'info') {
    self.postMessage({ type: 'info', info, crossOriginIsolated: self.crossOriginIsolated });
    return;
  }
  if (msg.type === 'run') {
    const m = modules[msg.engine];
    if (!m) { self.postMessage({ type: 'result', job: msg, result: { error: `engine ${msg.engine} not loaded` } }); return; }
    let result;
    try {
      result = JSON.parse(m.run_scenario(msg.engine, msg.scenario, msg.warmup, msg.ticks, false));
    } catch (err) {
      result = { error: String(err) };
    }
    self.postMessage({ type: 'result', job: msg, result });
  }
};
