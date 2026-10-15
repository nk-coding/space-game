// The wiki's theme: VitePress's default theme for its structure (sidebar, outline, prev/next, local search, the
// phone layout), restyled to look like the game (style.css, colours from art/tokens.json through gen/tokens.css),
// plus the components that show the game's data (gen/ui.json).
import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import Layout from './Layout.vue'
import HomePage from './components/HomePage.vue'
import PartCard from './components/PartCard.vue'
import PartGrid from './components/PartGrid.vue'
import PartsTable from './components/PartsTable.vue'
import ShipCard from './components/ShipCard.vue'
import ShipGallery from './components/ShipGallery.vue'
import ControlsTable from './components/ControlsTable.vue'
import NodeTable from './components/NodeTable.vue'
import StatTable from './components/StatTable.vue'
import KindGrid from './components/KindGrid.vue'
import ScrollBox from './components/ScrollBox.vue'
import '../../gen/tokens.css'
import './style.css'

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app }) {
    // Keep this list in step with COMPONENTS in content-tool's wiki/hand.rs, which checks their attributes.
    const components = { HomePage, PartCard, PartGrid, PartsTable, ShipCard, ShipGallery, ControlsTable, NodeTable, StatTable, KindGrid, ScrollBox }
    for (const [name, c] of Object.entries(components)) app.component(name, c)
  },
} satisfies Theme
