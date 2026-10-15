<script setup lang="ts">
// Every kind of content with how many entries it has, linked to its list.
import { withBase } from 'vitepress'
import { kinds } from '../data'
</script>

<template>
  <div class="sg-kinds">
    <a v-for="k in kinds" :key="k.kind" class="kind" :href="withBase(k.link)"><span class="n">{{ k.count }}</span><span class="t">{{ k.title }}</span></a>
  </div>
</template>
