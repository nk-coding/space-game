<script lang="ts" setup>
// VitePress's VPSidebar (1.6.4) with its scrolling in a ScrollBox: the aside no longer scrolls itself, the ScrollBox
// below the nav bar does. Swapped in for VPSidebar.vue by an alias in config.mts.
// Modified copy of VitePress 1.6.4 (https://github.com/vuejs/vitepress), MIT License, Copyright (c) 2019-present,
// Yuxi (Evan) You: see LICENSE-vitepress in this folder.
import { useScrollLock } from '@vueuse/core'
import { inBrowser } from 'vitepress'
import { ref, watch } from 'vue'
import { useSidebar } from 'vitepress/dist/client/theme-default/composables/sidebar.js'
import VPSidebarGroup from 'vitepress/dist/client/theme-default/components/VPSidebarGroup.vue'
import ScrollBox from '../components/ScrollBox.vue'

const { sidebarGroups, hasSidebar } = useSidebar()

const props = defineProps<{
  open: boolean
}>()

// a11y: focus Nav element when menu has opened
const navEl = ref<HTMLElement | null>(null)
const isLocked = useScrollLock(inBrowser ? document.body : null)

watch(
  [props, navEl],
  () => {
    if (props.open) {
      isLocked.value = true
      navEl.value?.focus()
    } else isLocked.value = false
  },
  { immediate: true, flush: 'post' }
)

const key = ref(0)

watch(
  sidebarGroups,
  () => {
    key.value += 1
  },
  { deep: true }
)
</script>

<template>
  <aside v-if="hasSidebar" class="VPSidebar" :class="{ open }" ref="navEl" @click.stop>
    <ScrollBox class="sidebar-scroll">
      <nav class="nav" id="VPSidebarNav" aria-labelledby="sidebar-aria-label" tabindex="-1">
        <span class="visually-hidden" id="sidebar-aria-label">Sidebar Navigation</span>
        <slot name="sidebar-nav-before" />
        <VPSidebarGroup :items="sidebarGroups" :key="key" />
        <slot name="sidebar-nav-after" />
      </nav>
    </ScrollBox>
  </aside>
</template>

<style scoped>
.VPSidebar {
  position: fixed;
  top: var(--vp-layout-top-height, 0px);
  bottom: 0;
  left: 0;
  z-index: var(--vp-z-index-sidebar);
  display: flex;
  flex-direction: column;
  width: calc(100vw - 64px);
  max-width: 320px;
  background-color: var(--vp-sidebar-bg-color);
  opacity: 0;
  box-shadow: var(--vp-c-shadow-3);
  overflow: hidden;
  transform: translateX(-100%);
  transition: opacity 0.5s, transform 0.25s ease;
}

.VPSidebar.open {
  opacity: 1;
  visibility: visible;
  transform: translateX(0);
  transition: opacity 0.25s, transform 0.5s cubic-bezier(0.19, 1, 0.22, 1);
}

.sidebar-scroll {
  flex: 1;
  min-height: 0;
}

.sidebar-scroll :deep(.sg-scroll-viewport) {
  overscroll-behavior: contain;
}

.nav {
  outline: 0;
  padding: 32px 32px 96px;
}

@media (min-width: 960px) {
  .VPSidebar {
    padding-top: var(--vp-nav-height);
    width: var(--vp-sidebar-width);
    max-width: 100%;
    opacity: 1;
    visibility: visible;
    box-shadow: none;
    transform: translateX(0);
  }
  .nav {
    padding-top: 24px;
  }
}

@media (min-width: 1440px) {
  .nav {
    padding-left: max(32px, calc((100vw - (var(--vp-layout-max-width) - 64px)) / 2));
  }
  .VPSidebar {
    width: calc((100% - (var(--vp-layout-max-width) - 64px)) / 2 + var(--vp-sidebar-width) - 32px);
  }
}
</style>
