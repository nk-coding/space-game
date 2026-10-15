<script setup lang="ts">
// Every part in one table: filter by name or category, sort by any column (the parts list's overview).
import { computed, ref } from 'vue'
import { parts, categories, stats, href, html } from '../data'

const cols = ['build_points', 'mass', 'hp', 'power_output', 'power_draw', 'heat_output', 'control_cost']
const query = ref('')
const category = ref<string | null>(null)
const sortBy = ref<string>('')
const desc = ref(true)
const value = (p: (typeof parts)[number], stat: string) => p.stats.find((s) => s.stat === stat)
const rows = computed(() => {
  const q = query.value.trim().toLowerCase()
  let list = parts.filter(
    (p) =>
      (!category.value || p.category === category.value) &&
      (!q || p.name.toLowerCase().includes(q) || p.summary.toLowerCase().includes(q)),
  )
  const key = sortBy.value
  if (key === 'name') list = [...list].sort((a, b) => a.name.localeCompare(b.name) * (desc.value ? -1 : 1))
  else if (key) {
    const v = (p: (typeof parts)[number]) => value(p, key)?.value ?? -Infinity
    list = [...list].sort((a, b) => (desc.value ? v(b) - v(a) : v(a) - v(b)))
  }
  return list
})
function sort(c: string) {
  if (sortBy.value === c) desc.value = !desc.value
  else [sortBy.value, desc.value] = [c, c !== 'name']
}
const catName = (id: string) => categories.find((c) => c.id === id)
</script>

<template>
  <div class="sg-parts-table">
    <div class="filters">
      <input v-model="query" type="search" placeholder="FILTER PARTS" aria-label="Filter parts" />
      <button :class="{ on: !category }" @click="category = null">All</button>
      <button
        v-for="c in categories"
        :key="c.id"
        :class="{ on: category === c.id }"
        :style="{ '--c': c.color ?? 'var(--sg-label)' }"
        @click="category = category === c.id ? null : c.id"
      ><i class="dot"></i>{{ c.name }}</button>
    </div>
    <ScrollBox x><table class="sg-table compare">
      <thead>
        <tr>
          <th></th>
          <th class="sortable" :class="{ on: sortBy === 'name' }" @click="sort('name')">Part</th>
          <th>Category</th>
          <th v-for="c in cols" :key="c" class="num sortable" :class="{ on: sortBy === c }" :title="stats[c]?.summary" @click="sort(c)">
            {{ stats[c]?.name ?? c }}<span v-if="sortBy === c">{{ desc ? ' ▾' : ' ▴' }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="p in rows" :key="p.id">
          <td class="pic" v-html="p.svg"></td>
          <td class="name">
            <a :href="href(p.url)">{{ p.name }}</a><b v-if="p.locked" class="badge locked">Locked</b>
            <span class="note" v-html="html(p.summary)"></span>
          </td>
          <td class="cat" :style="{ '--c': catName(p.category)?.color ?? 'var(--sg-label)' }"><i class="dot"></i>{{ catName(p.category)?.name }}</td>
          <td v-for="c in cols" :key="c" class="num">{{ value(p, c)?.text ?? '' }}</td>
        </tr>
      </tbody>
    </table></ScrollBox>
    <p class="note">{{ rows.length }} of {{ parts.length }} parts. Hull parts show their numbers in {{ parts.find((p) => p.hull)?.material }}; their pages list every armour.</p>
  </div>
</template>
