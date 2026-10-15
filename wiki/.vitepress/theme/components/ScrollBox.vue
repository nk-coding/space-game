<script setup lang="ts">
// Every scrolling region of the wiki (Niklas, 15 Oct 2026: no unstyled scrollbars): Reka UI's ScrollArea with thin
// bars in the game's look (style guide "Scrollbars": label grey, teal under the pointer and while dragged). The
// viewport is a native overflow element, so wheel, keyboard, touch and anchor jumps keep working.
import { ScrollAreaRoot, ScrollAreaScrollbar, ScrollAreaThumb, ScrollAreaViewport } from 'reka-ui'

withDefaults(defineProps<{ x?: boolean; y?: boolean }>(), { x: false, y: false })
</script>

<template>
  <ScrollAreaRoot class="sg-scroll" type="hover" :scroll-hide-delay="900">
    <ScrollAreaViewport class="sg-scroll-viewport">
      <slot />
    </ScrollAreaViewport>
    <ScrollAreaScrollbar v-if="y || !x" class="sg-scrollbar" orientation="vertical">
      <ScrollAreaThumb class="sg-thumb" />
    </ScrollAreaScrollbar>
    <ScrollAreaScrollbar v-if="x" class="sg-scrollbar" orientation="horizontal">
      <ScrollAreaThumb class="sg-thumb" />
    </ScrollAreaScrollbar>
  </ScrollAreaRoot>
</template>
