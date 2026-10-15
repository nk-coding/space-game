<script setup lang="ts">
// Every ship of the content as a small card.
import { ships } from '../data'
import ShipCard from './ShipCard.vue'
</script>

<template>
  <div class="sg-grid ships">
    <ShipCard v-for="s in ships" :id="s.id" :key="s.id" compact />
  </div>
</template>
