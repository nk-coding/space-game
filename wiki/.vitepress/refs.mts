// The Markdown side of "no duplication": hand-written pages name the game's data, this plugin fills it in from
// gen/refs.json (written by `content-tool wiki-data`, which also checks every reference and example; an unknown one
// here means gen/ is stale: run `npm run gen`).
//
//   [[part:thruster]]            a link labelled with the entry's name
//   [[part:thruster|thrusters]]  the same link with a label
//   [[part:thruster.thrust]]     a value as the game shows it ("200 kN")
//   [[sim:tick_hz]]              a constant of the simulation ("30 Hz")
//   ```behavior / ```assists     a rule list: its linked text and its graph, rendered by content-tool
import type MarkdownIt from 'markdown-it'

type Refs = {
  root: string
  entries: Record<string, { name: string; url: string }>
  values: Record<string, string>
  sim: Record<string, string>
  examples: Record<string, { kind: string; html: string }>
}

const esc = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

// Injected HTML goes through Vue's template compiler: braces must not read as interpolation.
const vueSafe = (html: string) => html.replace(/\{/g, '&#123;').replace(/\}/g, '&#125;')

export function refsPlugin(md: MarkdownIt, refs: Refs, base: string) {
  const fill = (html: string) => vueSafe(html.split(refs.root).join(base))

  const render = (ref: string, where: string): string => {
    const [target, label] = ref.split('|', 2)
    if (target.startsWith('sim:')) {
      const v = refs.sim[target.slice(4)]
      if (v === undefined) throw new Error(`${where}: [[${ref}]]: unknown sim constant (run npm run gen)`)
      return `<span class="sg-value">${esc(v)}</span>`
    }
    const colon = target.indexOf(':')
    const rest = target.slice(colon + 1)
    if (/[.@]/.test(rest)) {
      const v = refs.values[target]
      if (v === undefined) throw new Error(`${where}: [[${ref}]]: no such value (run npm run gen)`)
      return `<span class="sg-value">${fill(v)}</span>`
    }
    const e = refs.entries[target]
    if (!e) throw new Error(`${where}: [[${ref}]]: no such entry (run npm run gen)`)
    return `<a class="sg-ref" href="${base}${e.url}">${vueSafe(esc(label ?? e.name))}</a>`
  }

  // A line break between two references (`[[a]]\n[[b]]`) is a whitespace-only text node between two elements, which
  // Vue's template compiler drops when it holds a line break: write the break as a plain space.
  md.renderer.rules.softbreak = () => ' '

  md.inline.ruler.before('link', 'sg_ref', (state, silent) => {
    const src = state.src
    const pos = state.pos
    if (src.charCodeAt(pos) !== 0x5b || src.charCodeAt(pos + 1) !== 0x5b) return false
    const end = src.indexOf(']]', pos + 2)
    if (end < 0) return false
    if (!silent) {
      const where = (state.env as { relativePath?: string }).relativePath ?? 'a page'
      const token = state.push('html_inline', '', 0)
      token.content = render(src.slice(pos + 2, end), where)
    }
    state.pos = end + 2
    return true
  })

  const fence = md.renderer.rules.fence!
  md.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const token = tokens[idx]
    const lang = token.info.trim().split(/\s+/)[0]
    if (lang !== 'behavior' && lang !== 'assists') return fence(tokens, idx, options, env, self)
    const ex = refs.examples[token.content.replace(/\s+$/, '')]
    if (!ex) {
      throw new Error(`${env.relativePath}: a ${lang} example that gen/refs.json lacks (run npm run gen)`)
    }
    return `<ScrollBox x><div class="sg-example sg-example-${lang}" v-pre>${fill(ex.html)}</div></ScrollBox>\n`
  }
}

// What can be wider than the page scrolls sideways in a ScrollBox (theme/components/ScrollBox.vue): tables and the
// lines of code blocks.
export function scrollPlugin(md: MarkdownIt) {
  md.renderer.rules.table_open = () => '<ScrollBox x class="sg-table-scroll"><table>\n'
  md.renderer.rules.table_close = () => '</table></ScrollBox>\n'
  const fence = md.renderer.rules.fence!
  md.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const html = fence(tokens, idx, options, env, self)
    const start = html.indexOf('<pre')
    const end = html.lastIndexOf('</pre>')
    if (start < 0 || end < 0 || html.startsWith('<ScrollBox')) return html
    return `${html.slice(0, start)}<ScrollBox x class="sg-code-scroll">${html.slice(start, end + 6)}</ScrollBox>${html.slice(end + 6)}`
  }
}
