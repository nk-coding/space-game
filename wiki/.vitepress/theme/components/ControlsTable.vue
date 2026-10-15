<script setup lang="ts">
// One section of the action map (controls.ron): each action with its default keys and what it does.
import { computed } from 'vue'
import { controls, sections, href, html } from '../data'

const props = defineProps<{ section: string }>()
const rows = computed(() => controls.filter((c) => c.section === props.section))
const title = computed(() => sections.find((s) => s.id === props.section)?.title)
</script>

<template>
  <ScrollBox x><table class="sg-table controls" :aria-label="title">
    <thead><tr><th>Action</th><th>Default</th><th>What it does</th></tr></thead>
    <tbody>
      <tr v-for="c in rows" :key="c.id">
        <td class="name"><a :href="href(c.url)">{{ c.name }}</a></td>
        <td class="keys" v-html="c.keys"></td>
        <td v-html="html(c.summary)"></td>
      </tr>
    </tbody>
  </table></ScrollBox>
</template>
