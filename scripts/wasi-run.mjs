// Runs a wasm32-wasip1 binary under Node's built-in WASI (preview1) with the given arguments.
// stdout/stderr go to this process; the exit code is passed through. No filesystem access is granted.
// Usage: node scripts/wasi-run.mjs <file.wasm> [args...]
import { readFile } from 'node:fs/promises';
import { WASI } from 'node:wasi';
import process from 'node:process';

const [file, ...args] = process.argv.slice(2);
if (!file) {
  console.error('usage: node scripts/wasi-run.mjs <file.wasm> [args...]');
  process.exit(2);
}
const wasi = new WASI({ version: 'preview1', args: [file, ...args], env: {}, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
