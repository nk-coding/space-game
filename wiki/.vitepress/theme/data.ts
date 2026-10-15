// What the components show: gen/ui.json from content-tool wiki-data. Its HTML links through a placeholder root,
// replaced here with the site's base so a component works on a page at any depth.
import { withBase } from 'vitepress'
import ui from '../../gen/ui.json'

export type StatRow = { stat: string; text: string; value?: number }
export type Part = {
  id: string; name: string; url: string; category: string; locked: boolean; hull: boolean; material: string | null
  size: [number, number]; summary: string; svg: string; stats: StatRow[]
}
export type Ship = { id: string; name: string; url: string; faction: string; summary: string; svg: string; stats: StatRow[] }
export type Control = { id: string; name: string; url: string; section: string; summary: string; keys: string }
export type Node = {
  id: string; name: string; url: string; role: 'tgt' | 'cond' | 'act'; pilot: boolean; rhai: boolean; tags: string[]
  summary: string; signature: string
}
export type Stat = { name: string; url: string; unit: string; better: string; summary: string }

const fix = (html: string) => html.split(ui.root).join(withBase('/'))

export const href = (url: string) => withBase('/' + url)
export const html = fix
export const kinds = ui.kinds as { kind: string; title: string; link: string; count: number }[]
export const categories = ui.categories as { id: string; name: string; url: string; color: string | null }[]
export const stats = ui.stats as Record<string, Stat>
export const parts = ui.parts as Part[]
export const ships = ui.ships as Ship[]
export const controls = ui.controls as Control[]
export const sections = ui.sections as { id: string; title: string }[]
export const nodes = ui.nodes as Node[]

export const part = (id: string) => parts.find((p) => p.id === id)
export const ship = (id: string) => ships.find((s) => s.id === id)
