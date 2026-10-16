<script setup lang="ts">
// Behavior nodes of one role: how each is written and what it does. Without `pilot` the nodes a behavior may use;
// with it the pilot assists' own (GDR-0088).
import { computed } from 'vue'
import { nodes, href, html } from '../data'

const props = defineProps<{ role?: 'target' | 'condition' | 'action'; pilot?: boolean; tag?: string }>()
const roles = { target: 'tgt', condition: 'cond', action: 'act' } as const
const rows = computed(() =>
  nodes.filter(
    (n) =>
      (!props.role || n.role === roles[props.role]) &&
      (props.tag ? n.tags.includes(props.tag) : n.pilot === !!props.pilot),
  ),
)
</script>

<template>
  <ScrollBox x><table class="sg-table nodes">
    <thead><tr><th>Node</th><th>Written</th><th>What it does</th></tr></thead>
    <tbody>
      <tr v-for="n in rows" :key="n.id">
        <td class="name"><a :class="n.role" :href="href(n.url)">{{ n.name }}</a><b v-if="n.rhai" class="badge rhai" title="A condition written as a small Rhai script, not built into the game">Script</b></td>
        <td><code :class="n.role">{{ n.signature }}</code></td>
        <td v-html="html(n.summary)"></td>
      </tr>
    </tbody>
  </table></ScrollBox>
</template>
