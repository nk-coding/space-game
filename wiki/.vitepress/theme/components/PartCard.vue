<script setup lang="ts">
// A part as a card: its palette tile, name, category, summary and stats, linked to its page.
import { computed } from 'vue'
import { part, stats, categories, href, html } from '../data'

const props = defineProps<{ id: string; compact?: boolean }>()
const p = computed(() => part(props.id))
const category = computed(() => categories.find((c) => c.id === p.value?.category))
const rows = computed(() => (p.value ? p.value.stats.filter((s) => s.stat !== 'build_points').slice(0, props.compact ? 3 : 6) : []))
</script>

<template>
  <a v-if="p" class="sg-card part-card" :href="href(p.url)">
    <span class="tile" v-html="p.svg"></span>
    <span class="body">
      <span class="kicker" :style="{ '--c': category?.color ?? 'var(--sg-label)' }"><i class="dot"></i>{{ category?.name }}<b v-if="p.locked" class="badge locked">Locked</b></span>
      <span class="title">{{ p.name }}</span>
      <span class="sub" v-html="html(p.summary)"></span>
      <span class="stats">
        <span v-for="s in rows" :key="s.stat" class="stat"><span class="l">{{ stats[s.stat]?.name ?? s.stat }}</span><span class="v">{{ s.text }}</span></span>
      </span>
    </span>
  </a>
</template>
