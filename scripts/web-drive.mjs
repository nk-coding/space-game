#!/usr/bin/env node
// Drives the web client in headless Chromium (WebGL2 via SwiftShader) with scripted input and screenshots.
// Uses the Playwright install of tools/headless (set HEADLESS_DIR to point elsewhere).
//
// Usage: node scripts/web-drive.mjs <url> <out-dir> "<steps>" [width] [height]
// Steps, separated by ';':
//   wait <ms> | down <key> | up <key> | press <key> | type <text>
//   move <x> <y> | mousedown <x> <y> [left|right|middle] | mouseup [left|right|middle] | click <x> <y>
//   wheel <dy> | tap <x> <y> | touchdrag <x0> <y0> <x1> <y1> <holdMs> (one finger, CDP touch events)
//   shot <file.png> | crop <x> <y> <w> <h> <file.png> (a part of the page; enlarge with scripts/png-zoom.py)
//   eval <js> | reload (the page again, same storage)
//   upload <file> (the next file chooser gets this file) | clip <text> (put text on the clipboard) | readclip
// Downloads are saved into <out-dir> under their suggested names ("[download] <path>" is printed).
// Keys use Playwright names (KeyW, Space, Tab, Digit1, F3, ShiftLeft...).
// Console lines are printed; `--quiet-console` drops lines that don't look like errors or hash lines.
// `--dpr=N` emulates a phone like Chrome DevTools' device mode: device scale factor N (also forced on Chromium,
// docs/client.md "Headless HiDPI"), mobile viewport, touch; e.g. `--dpr=3` with 844 390 (docs/client.md "Phones"). Its `shot`s are saved at CSS size
// (844 × 390, the phone's dp: what to read the layout from); `crop` keeps the device pixels for close-ups.
import { createRequire } from 'node:module';
import path from 'node:path';
import fs from 'node:fs';

const dir = process.env.HEADLESS_DIR || '/home/devbox/workspace/tools/headless';
const require = createRequire(path.join(dir, 'package.json'));
const { chromium } = require('playwright');

const args = process.argv.slice(2).filter(a => !a.startsWith('--'));
const quiet = process.argv.includes('--quiet-console');
const dpr = Number((process.argv.find(a => a.startsWith('--dpr=')) || '--dpr=0').slice(6));
const [url, outDir, steps = '', w = '1280', h = '720'] = args;
if (!url || !outDir) {
  console.error('usage: web-drive.mjs <url> <out-dir> "<steps>" [width] [height]');
  process.exit(2);
}
fs.mkdirSync(outDir, { recursive: true });

const launchArgs = ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'];
if (dpr) launchArgs.push(`--force-device-scale-factor=${dpr}`);
const browser = await chromium.launch({ args: launchArgs });
const phone = dpr ? { deviceScaleFactor: dpr, isMobile: true } : {};
const ctx = await browser.newContext({ viewport: { width: +w, height: +h }, hasTouch: true, acceptDownloads: true, ...phone });
await ctx.grantPermissions(['clipboard-read', 'clipboard-write']).catch(e => console.log(`[clipboard] ${e.message}`));
const page = await ctx.newPage();
page.on('download', async d => {
  const to = path.join(outDir, d.suggestedFilename());
  await d.saveAs(to);
  console.log(`[download] ${to}`);
});
page.on('console', m => {
  const t = m.text();
  if (quiet && !/error|panic|warn|^\d+ [0-9a-f]{16}$|^#|LOADTEST/i.test(t)) return;
  console.log(t.startsWith('%c') ? `[console.${m.type()}] ${t.replace(/%c/g, '')}` : t);
});
page.on('pageerror', e => console.log(`[pageerror] ${e.message}`));
await page.goto(url, { waitUntil: 'load', timeout: 120000 });
const cdp = await ctx.newCDPSession(page);

for (const raw of steps.split(';').map(s => s.trim()).filter(Boolean)) {
  const [cmd, ...a] = raw.split(/\s+/);
  const n = i => Number(a[i]);
  switch (cmd) {
    case 'wait': await page.waitForTimeout(n(0)); break;
    case 'down': await page.keyboard.down(a[0]); break;
    case 'up': await page.keyboard.up(a[0]); break;
    case 'press': await page.keyboard.press(a[0]); break;
    case 'type': await page.keyboard.type(a.join(' ')); break;
    case 'move': await page.mouse.move(n(0), n(1), { steps: 4 }); break;
    case 'mousedown': await page.mouse.move(n(0), n(1)); await page.mouse.down({ button: a[2] || 'left' }); break;
    case 'mouseup': await page.mouse.up({ button: a[0] || 'left' }); break;
    case 'click': await page.mouse.click(n(0), n(1)); break;
    case 'wheel': await page.mouse.wheel(0, n(0)); break;
    case 'tap': await page.touchscreen.tap(n(0), n(1)); break;
    case 'touchdrag': {
      const [x0, y0, x1, y1, hold] = a.map(Number);
      const pt = (x, y) => [{ x, y, id: 7 }];
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: pt(x0, y0) });
      for (let k = 1; k <= 6; k++) {
        await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: pt(x0 + (x1 - x0) * k / 6, y0 + (y1 - y0) * k / 6) });
        await page.waitForTimeout(60);
      }
      await page.waitForTimeout(hold || 1000);
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      break;
    }
    case 'shot': await page.screenshot({ path: path.join(outDir, a[0]), scale: dpr ? 'css' : 'device' }); console.log(`[shot] ${path.join(outDir, a[0])}`); break;
    case 'crop': {
      const to = path.join(outDir, a[4]);
      await page.screenshot({ path: to, clip: { x: n(0), y: n(1), width: n(2), height: n(3) } });
      console.log(`[crop] ${to}`);
      break;
    }
    case 'eval': console.log(await page.evaluate(a.join(' '))); break;
    case 'reload': await page.reload({ waitUntil: 'load', timeout: 120000 }); break;
    case 'upload': page.once('filechooser', fc => fc.setFiles(a[0])); break;
    case 'clip': await page.evaluate(t => navigator.clipboard.writeText(t), a.join(' ')); break;
    case 'readclip': console.log(`[clipboard] ${await page.evaluate(() => navigator.clipboard.readText())}`); break;
    default: console.log(`unknown step: ${raw}`);
  }
}
await browser.close();
