// The built wiki's completeness check (npm run check, after npm run build; docs/wiki.md "Checks"). content-tool
// wiki-data has already checked the sources (every reference, example, component and navigation entry, nothing raster);
// this checks what the build made of them:
// - every content entry (gen/refs.json) and every hand-written page (nav.json) has its page;
// - every link and asset reference inside the site resolves, fragments included;
// - nothing raster: no raster file, no <img>, no raster url();
// - the search index holds every page.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const dist = path.join(root, '.vitepress', 'dist')
const base = process.env.WIKI_BASE ?? '/wiki/'
const problems = []
const fail = (m) => problems.push(m)

if (!fs.existsSync(dist)) {
  console.error('no .vitepress/dist: run npm run build first')
  process.exit(1)
}
const files = []
const walk = (d) => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name)
    if (e.isDirectory()) walk(p)
    else files.push(path.relative(dist, p).split(path.sep).join('/'))
  }
}
walk(dist)
const have = new Set(files)

// Every entry and every hand-written page has its page (an entry of the lab's page its row there, below).
const refs = JSON.parse(fs.readFileSync(path.join(root, 'gen', 'refs.json'), 'utf8'))
for (const [key, e] of Object.entries(refs.entries)) if (!have.has(e.url.split('#')[0])) fail(`no page for ${key} (${e.url})`)
const nav = JSON.parse(fs.readFileSync(path.join(root, 'nav.json'), 'utf8'))
const handPages = ['index', ...nav.flatMap((s) => s.items)]
for (const p of handPages) if (!have.has(`${p}.html`)) fail(`no page for the hand-written ${p}.md`)

// Nothing raster.
const raster = /\.(png|jpe?g|gif|webp|avif|bmp|ico|tiff?|heic|jxl)$/i
for (const f of files) if (raster.test(f)) fail(`a raster file: ${f}`)

// Links and references inside the site resolve.
const ids = new Map()
const idsOf = (f) => {
  if (!ids.has(f)) {
    const text = fs.readFileSync(path.join(dist, f), 'utf8')
    ids.set(f, new Set([...text.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])))
  }
  return ids.get(f)
}
for (const [key, e] of Object.entries(refs.entries)) {
  const [page, anchor] = e.url.split('#')
  if (anchor && have.has(page) && !idsOf(page).has(anchor)) fail(`no anchor for ${key} (${e.url})`)
}
const pages = files.filter((f) => f.endsWith('.html'))
let links = 0
for (const f of pages) {
  const text = fs.readFileSync(path.join(dist, f), 'utf8')
  if (/<img\b/i.test(text)) fail(`${f}: an <img>`)
  for (const m of text.matchAll(/\s(?:href|src)="([^"]*)"/g)) {
    const url = m[1].replace(/&amp;/g, '&')
    if (/^(https?:|mailto:|data:|javascript:)/.test(url)) {
      if (/^data:image\/(?!svg)/.test(url)) fail(`${f}: a raster data URI`)
      continue
    }
    links++
    const [target, frag] = url.split('#')
    let file
    if (!target) file = f
    else if (target.startsWith(base)) file = target.slice(base.length)
    else if (target.startsWith('/')) {
      fail(`${f}: a link outside the base: ${url}`)
      continue
    } else file = path.posix.normalize(path.posix.join(path.posix.dirname(f), target))
    if (file === '' || file.endsWith('/')) file += 'index.html'
    if (!have.has(file)) {
      fail(`${f}: broken link ${url}`)
      continue
    }
    if (frag && file.endsWith('.html') && !idsOf(file).has(decodeURIComponent(frag))) fail(`${f}: no #${frag} in ${file}`)
  }
}
for (const f of files.filter((f) => f.endsWith('.css'))) {
  const text = fs.readFileSync(path.join(dist, f), 'utf8')
  for (const m of text.matchAll(/url\(([^)]*)\)/g)) if (raster.test(m[1].replace(/["']/g, '').split('?')[0])) fail(`${f}: a raster url(${m[1]})`)
}

// The search index holds every page.
const index = files.find((f) => /@localSearchIndexroot\.[\w-]+\.js$/.test(f))
if (!index) fail('no local search index')
else {
  const text = fs.readFileSync(path.join(dist, index), 'utf8')
  for (const p of pages.filter((p) => p !== '404.html')) {
    // A directory's index page is indexed by its directory: /wiki/parts/#parts.
    const id = p.endsWith('index.html') ? `${base}${p.slice(0, -'index.html'.length)}#` : `${base}${p}`
    if (!text.includes(id)) fail(`the search index lacks ${p}`)
  }
}

if (problems.length) {
  for (const p of problems) console.error(`error: ${p}`)
  console.error(`${problems.length} problem(s)`)
  process.exit(1)
}
console.log(`ok: ${pages.length} pages, ${links} internal links, ${Object.keys(refs.entries).length} entries, ${handPages.length} hand-written pages, all in the search index; nothing raster`)
