// The wiki's VitePress config (ADR-0039, docs/wiki.md). Two halves:
// - hand-written pages (guide/, mechanics/, behavior/, listed in nav.json): prose that references the game's data by
//   id ([[kind:id]], [[kind:id.field]], [[sim:name]]) and rule examples (```behavior, ```assists), resolved here from
//   gen/refs.json;
// - generated pages (gen/<kind dir>/*.md, written by `content-tool wiki-data`): one per content entry, served at
//   /<kind dir>/<id>.html through the rewrites below.
// `npm run gen` writes gen/; every build runs it first, so the data never lags behind the content.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig, type DefaultTheme } from 'vitepress'
import { refsPlugin, scrollPlugin } from './refs.mts'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const gen = path.join(root, 'gen')
if (!fs.existsSync(path.join(gen, 'refs.json'))) {
  throw new Error('wiki/gen is missing: run `npm run gen` (content-tool wiki-data) first')
}
const readJson = (file: string) => JSON.parse(fs.readFileSync(file, 'utf8'))

// The site lives under /wiki/ next to the game (http://localhost:8080/wiki/); WIKI_BASE overrides it.
const base = process.env.WIKI_BASE ?? '/wiki/'

// Generated pages: gen/parts/thruster.md is served as parts/thruster.html.
const rewrites: Record<string, string> = {}
const walk = (dir: string) => {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) walk(p)
    else if (e.name.endsWith('.md')) {
      const rel = path.relative(root, p).split(path.sep).join('/')
      rewrites[rel] = rel.slice('gen/'.length)
    }
  }
}
walk(gen)

// The hand-written navigation: nav.json lists the pages; each page's front matter gives its title.
type NavSection = { text: string; items: string[] }
const nav: NavSection[] = readJson(path.join(root, 'nav.json'))
const titleOf = (page: string) => {
  const text = fs.readFileSync(path.join(root, `${page}.md`), 'utf8')
  const m = /^title:\s*(.+)$/m.exec(text)
  return m ? m[1].trim().replace(/^["']|["']$/g, '') : page
}
const linkOf = (page: string) => '/' + page.replace(/(^|\/)index$/, '$1')
const handSidebar: DefaultTheme.SidebarItem[] = nav.map((s) => ({
  text: s.text,
  collapsed: false,
  items: s.items.map((p) => ({ text: titleOf(p), link: linkOf(p) })),
}))
const reference: DefaultTheme.SidebarItem = {
  text: 'Reference',
  link: '/reference/',
  collapsed: false,
  items: readJson(path.join(gen, 'sidebar.json')),
}

export default defineConfig({
  base,
  lang: 'en',
  title: 'Space Game wiki',
  titleTemplate: ':title · Space Game wiki',
  description: 'The ship-mind’s handbook: every part, mechanic and behavior of Space Game, from the game’s own data.',
  cleanUrls: false,
  rewrites,
  srcExclude: ['README.md', 'scripts/**'],
  lastUpdated: false,
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: `${base}favicon.svg` }],
    ['meta', { name: 'theme-color', content: '#050e15' }],
  ],
  // The game is dark; so is its wiki.
  appearance: 'force-dark',
  markdown: {
    // Content files are RON: Rust's highlighting reads it well enough.
    languageAlias: { ron: 'rust' },
    config: (md) => {
      scrollPlugin(md)
      refsPlugin(md, readJson(path.join(gen, 'refs.json')), base)
    },
  },
  themeConfig: {
    siteTitle: 'SPACE GAME',
    nav: [
      { text: 'Guide', link: '/guide/getting-started', activeMatch: '^/guide/' },
      { text: 'Mechanics', link: '/mechanics/flight', activeMatch: '^/mechanics/' },
      { text: 'Behavior', link: '/behavior/', activeMatch: '^/behavior/' },
      { text: 'Parts', link: '/parts/', activeMatch: '^/parts/' },
      {
        text: 'Reference',
        link: '/reference/',
        activeMatch: '^/(reference|materials|categories|ships|frames|factions|research|families|facilities|starts|voyages|encounters|behaviors|behavior_nodes|stats|controls|pages)/',
      },
    ],
    sidebar: [...handSidebar, reference],
    outline: { level: [2, 3], label: 'On this page' },
    docFooter: { prev: 'Previous', next: 'Next' },
    search: {
      provider: 'local',
      options: {
        detailedView: true,
        miniSearch: { searchOptions: { fuzzy: 0.2, prefix: true, boost: { title: 4, text: 2, titles: 1 } } },
      },
    },
    footer: {
      message: 'Generated from the game’s content packs by content-tool wiki-data. Vector graphics only.',
    },
  },
  vite: {
    server: { fs: { allow: [root] } },
    // Every scrolling region scrolls in a ScrollBox (Reka UI's ScrollArea): these default-theme components are
    // swapped for copies that wrap their scrolling part in one (theme/overrides/).
    resolve: {
      alias: Object.entries({
        VPSidebar: 'SgSidebar',
        VPDoc: 'SgDoc',
        VPNavScreen: 'SgNavScreen',
        VPLocalSearchBox: 'SgLocalSearchBox',
        VPLocalNavOutlineDropdown: 'SgLocalNavOutlineDropdown',
      }).map(([from, to]) => ({
        find: new RegExp(`^.*/${from}\\.vue$`),
        replacement: fileURLToPath(new URL(`./theme/overrides/${to}.vue`, import.meta.url)),
      })),
    },
  },
})
