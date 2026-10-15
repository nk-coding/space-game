<script setup lang="ts">
// Parts compared on a few stats: every part that has one of them, a column per stat; a click on a column sorts.
import { computed, ref } from 'vue'
import { parts, stats, href } from '../data'

const props = defineProps<{ stats: string }>()
const cols = computed(() => props.stats.split(',').map((s) => s.trim()))
const sortBy = ref<string | null>(null)
const desc = ref(true)
const rows = computed(() => {
  const list = parts.filter((p) => p.stats.some((s) => cols.value.includes(s.stat)))
  const key = sortBy.value
  if (!key) return list
  const v = (p: (typeof parts)[number]) => p.stats.find((s) => s.stat === key)?.value ?? -Infinity
  return [...list].sort((a, b) => (desc.value ? v(b) - v(a) : v(a) - v(b)))
})
const cell = (p: (typeof parts)[number], stat: string) => p.stats.find((s) => s.stat === stat)?.text ?? '—'
function sort(stat: string) {
  if (sortBy.value === stat) desc.value = !desc.value
  else [sortBy.value, desc.value] = [stat, true]
}
</script>

<template>
  <ScrollBox x><table class="sg-table compare">
    <thead>
      <tr>
        <th colspan="2">Part</th>
        <th v-for="c in cols" :key="c" class="num sortable" :class="{ on: sortBy === c }" :title="stats[c]?.summary" @click="sort(c)">
          {{ stats[c]?.name ?? c }}<span v-if="sortBy === c">{{ desc ? ' ▾' : ' ▴' }}</span>
        </th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="p in rows" :key="p.id">
        <td class="pic" v-html="p.svg"></td>
        <td class="name"><a :href="href(p.url)">{{ p.name }}</a></td>
        <td v-for="c in cols" :key="c" class="num">{{ cell(p, c) }}</td>
      </tr>
    </tbody>
  </table></ScrollBox>
</template>
