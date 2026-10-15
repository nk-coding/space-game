<script setup lang="ts">
// Cards of every part of a category (or of all parts), in the drydock palette's order.
import { computed } from 'vue'
import { parts } from '../data'
import PartCard from './PartCard.vue'

const props = defineProps<{ category?: string }>()
const list = computed(() => parts.filter((p) => !props.category || p.category === props.category))
</script>

<template>
  <div class="sg-grid">
    <PartCard v-for="p in list" :id="p.id" :key="p.id" compact />
  </div>
</template>
