<script setup lang="ts">
// A ship as a card: its drawing, faction, summary and the drydock's numbers (its design report).
import { computed } from 'vue'
import { ship, stats, href, html } from '../data'

const props = defineProps<{ id: string; compact?: boolean }>()
const s = computed(() => ship(props.id))
</script>

<template>
  <a v-if="s" class="sg-card ship-card" :class="{ compact }" :href="href(s.url)">
    <span class="drawing" v-html="s.svg"></span>
    <span class="body">
      <span class="kicker">{{ s.faction }}</span>
      <span class="title">{{ s.name }}</span>
      <span class="sub" v-html="html(s.summary)"></span>
      <span v-if="!compact" class="stats">
        <span v-for="r in s.stats" :key="r.stat" class="stat"><span class="l">{{ stats[r.stat]?.name ?? r.stat }}</span><span class="v">{{ r.text }}</span></span>
      </span>
    </span>
  </a>
</template>
