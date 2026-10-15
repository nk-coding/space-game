<script setup lang="ts">
// The home page's hero and its doors into the wiki's sections; the rest of the page is Markdown.
import { withBase } from 'vitepress'
import { kinds, parts, href } from '../data'

const count = (kind: string) => kinds.find((k) => k.kind === kind)?.count ?? 0
const doors = [
  { title: 'Guide', link: '/guide/getting-started.html', text: 'Who you are, the loop from the shipyard to the rim and back, and your first steps.' },
  { title: 'Mechanics', link: '/mechanics/flight.html', text: 'Flight, power and heat, damage, weapons, salvage, the drydock, voyages and research.' },
  { title: 'Behavior', link: '/behavior/', text: 'How ships think: rules, conditions, actions and targets, the rule text and the pilot assists.' },
  { title: 'Reference', link: '/reference/', text: 'Every part, ship, research node, voyage and control, generated from the game’s data.' },
]
</script>

<template>
  <section class="sg-hero">
    <p class="kicker">The ship-mind’s handbook</p>
    <h1 class="title">Space Game <span>wiki</span></h1>
    <p class="lead">
      You are an awakened ship-mind in an abandoned precursor shipyard at the rim of the galaxy. Build ships block by
      block, decide how they think, fly them yourself and salvage what you find.
    </p>
    <div class="actions">
      <a class="pill primary" :href="withBase('/guide/getting-started.html')">Getting started</a>
      <a class="pill" :href="withBase('/parts/')">{{ count('part') }} parts</a>
      <a class="pill" :href="withBase('/behavior/')">How ships think</a>
    </div>
    <div class="palette" aria-hidden="true">
      <a v-for="p in parts" :key="p.id" :href="href(p.url)" :title="p.name" v-html="p.svg"></a>
    </div>
  </section>
  <div class="sg-doors">
    <a v-for="d in doors" :key="d.title" class="door" :href="withBase(d.link)">
      <span class="title">{{ d.title }}</span>
      <span class="sub">{{ d.text }}</span>
    </a>
  </div>
</template>
