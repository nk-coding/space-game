<script setup lang="ts">
// The default layout plus what every page needs: the page's scrollbar, the icon sprite (the content pack's icons as symbols, for every
// drawing on the page), the arrowheads diagrams use, the game's backdrop (dotted rings and faint particles, as on
// its screens), and on an entry's page the kind it belongs to above its title.
import DefaultTheme from 'vitepress/theme'
import { useData, withBase } from 'vitepress'
import { computed } from 'vue'
import sprite from '../../gen/sprite.svg?raw'
import PageScrollbar from './components/PageScrollbar.vue'

const { frontmatter } = useData()
const entry = computed(
  () => frontmatter.value.entry as { kind: string; id: string; dir: string; kindTitle: string } | undefined,
)
const listLink = computed(() => (entry.value ? withBase(`/${entry.value.dir}/`) : ''))

// Particles of the backdrop: a fixed scatter (a small LCG, so server and browser draw the same).
const particles = (() => {
  let s = 7
  const r = () => ((s = (s * 48271) % 2147483647) / 2147483647)
  return Array.from({ length: 70 }, () => ({ x: r() * 1600, y: r() * 1000, o: 0.15 + r() * 0.5, d: 0.6 + r() * 1.4 }))
})()
const Layout = DefaultTheme.Layout
</script>

<template>
  <Layout>
    <template #layout-top>
      <div class="sg-sprite" aria-hidden="true" v-html="sprite"></div>
      <svg class="sg-sprite" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="0" height="0">
        <defs>
          <marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="f-dim" /></marker>
          <marker id="arrow-teal" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="f-teal" /></marker>
          <marker id="arrow-gold" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="f-gold" /></marker>
          <marker id="arrow-coral" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="f-coral" /></marker>
          <marker id="arrow-violet" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="f-violet" /></marker>
          <filter id="sg-glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="2.2" result="b" /><feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge></filter>
        </defs>
      </svg>
      <svg class="sg-backdrop" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
        <circle cx="1180" cy="380" r="330" class="ring" />
        <circle cx="1180" cy="380" r="520" class="ring" />
        <circle cx="1180" cy="380" r="760" class="ring faint" />
        <circle v-for="(p, i) in particles" :key="i" :cx="p.x" :cy="p.y" :r="p.d" :opacity="p.o" class="particle" />
      </svg>
    </template>
    <template #layout-bottom>
      <PageScrollbar />
    </template>
    <template #doc-before>
      <div v-if="entry" class="sg-kicker">
        <a :href="listLink">{{ entry.kindTitle }}</a>
        <span class="sep">›</span>
        <code>{{ entry.kind }}:{{ entry.id }}</code>
      </div>
    </template>
  </Layout>
</template>
