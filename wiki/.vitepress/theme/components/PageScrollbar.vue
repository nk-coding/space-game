<script setup lang="ts">
// The page's own scrollbar. The document keeps its native scrolling (VitePress's anchor jumps, search jumps and the
// outline's active heading all follow the window), with the native bar hidden; this draws the same thin bar as the
// ScrollBoxes (Reka UI's ScrollArea) over the right edge: shown while scrolling or under the pointer, draggable, a
// click on the track pages.
import { onBeforeUnmount, onMounted, ref } from 'vue'

const top = ref(0)
const size = ref(0)
const shown = ref(false)
const dragging = ref(false)
const track = ref<HTMLElement | null>(null)
let hide: ReturnType<typeof setTimeout> | undefined
let grab = 0

function measure() {
  const doc = document.documentElement
  const h = track.value?.clientHeight ?? window.innerHeight
  const total = doc.scrollHeight
  const view = window.innerHeight
  if (total <= view + 1) {
    size.value = 0
    return
  }
  size.value = Math.max(32, (view / total) * h)
  top.value = (window.scrollY / (total - view)) * (h - size.value)
}
function flash() {
  measure()
  shown.value = true
  clearTimeout(hide)
  hide = setTimeout(() => (shown.value = dragging.value), 1000)
}
function scrollToThumb(y: number) {
  const doc = document.documentElement
  const h = track.value?.clientHeight ?? window.innerHeight
  const share = Math.min(1, Math.max(0, y / (h - size.value)))
  window.scrollTo({ top: share * (doc.scrollHeight - window.innerHeight) })
}
function down(e: PointerEvent) {
  dragging.value = true
  grab = e.clientY - top.value
  ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
}
function move(e: PointerEvent) {
  if (dragging.value) scrollToThumb(e.clientY - grab)
}
function up() {
  dragging.value = false
  flash()
}
function page(e: MouseEvent) {
  if (e.target !== track.value) return
  const dir = e.clientY < top.value ? -1 : 1
  window.scrollBy({ top: dir * window.innerHeight * 0.9 })
}

let observer: ResizeObserver | undefined
onMounted(() => {
  window.addEventListener('scroll', flash, { passive: true })
  window.addEventListener('resize', measure)
  observer = new ResizeObserver(measure)
  observer.observe(document.body)
  measure()
})
onBeforeUnmount(() => {
  window.removeEventListener('scroll', flash)
  window.removeEventListener('resize', measure)
  observer?.disconnect()
  clearTimeout(hide)
})
</script>

<template>
  <div
    v-show="size > 0"
    ref="track"
    class="sg-page-scrollbar sg-scrollbar"
    :class="{ shown: shown || dragging }"
    data-orientation="vertical"
    aria-hidden="true"
    @pointerenter="flash"
    @click="page"
  >
    <div
      class="sg-thumb"
      :class="{ dragging }"
      :style="{ height: size + 'px', transform: `translateY(${top}px)` }"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="up"
      @pointercancel="up"
    ></div>
  </div>
</template>
