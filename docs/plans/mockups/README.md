# Mockups: the phase 7 screens (first playable loop)

Made 13 Oct 2026 for Niklas to judge the look of the new screens before they are built
([`../first-playable-loop.md`](../first-playable-loop.md) sections 3 and 5.1, [GDR-0062](../../gdr/0062-first-playable-loop.md),
[GDR-0063](../../gdr/0063-salvage-and-research-v1.md)). These are proposals, not decisions. All numbers are placeholders.

Each screen is a self-contained HTML file (inline SVG, Barlow embedded as a subset, no scripts, no external files)
with its PNG next to it. They are laid out at the client's 1280 × 720 logical size, like the drydock, and zoomed
to 1600 × 900 (what `UiScale` does on such a window). Colours come from `art/tokens.json`, part tiles from
`content/base/icons`, and the look follows `docs/concept/boards/ui-mockups/` and the real drydock and HUD
(`docs/results/drydock-*.png`, `behavior-*.png`). Ships: *Rim skiff* is the plan's starter (`sample_early` with a gyro
pair), *Rim skiff Mk II* has the reverse-engineered cannons, *Picket* and *Drone wing* are invented escort designs.

## Screens and variants

| Screen | File | What it shows |
|---|---|---|
| Home | `home.png` | Ring menu around the docked ship (DRYDOCK, BEHAVIOR, RESEARCH `1 READY`, VOYAGES as the primary button, TEST RANGE, FLEET); `THE SHIPYARD · RIM`; NEXT UNLOCK top right; the debrief as one line at the bottom left (`HAUL UNLOADED · 14 parts → KINETIC +18 · PROPULSION +9 · LOGIC +4 · HULL +4`) |
| Home B | `home-b.png` | The ring moves left and a full debrief column fills the right after a voyage: time, sites, kills, the parts dismantled, data per family, programmable matter, research ready, DISMISS / RESEARCH |
| Research | `research.png` | Families as columns; the data pool on top of each column; node cards in corner brackets (kind tag, cost chips gold when payable, progress bar, needs); states done / ready / short / blocked; the selected node's part preview, stats and UNLOCK on the right |
| Research B | `research-b.png` | The same nodes as a tree: one lane per family, tiers left to right, `requires` drawn as connectors (like the behavior graph's trunk) |
| Voyage map | `map.png` | The shipyard and the four rim voyages as a chain (cleared, open and selected, locked, the Warden's gate in coral); the destination panel: chips `RIM · AUTOMATONS · WRECKS`, sites, `≈ 10 MIN · RECALL WHEN CLEAR`, the fleet, the voyage-ready check, SET COURSE |
| Voyage map B | `map-b.png` | The same voyages placed as a star map (content positions) inside the rim band, uncharted clusters faint, the coral gate ring to the outer reach; the dotted route still reads as the chain |
| Voyage HUD | `voyage-hud.png` | A fight: `HALVEX DRIFT · RIM`, `SITES 2 / 5`, cargo `SALVAGE 5 / 8 · CARGO HOLD` under the hull, site markers on the edges (raiders coral, wrecks gold, the goal brighter with a diamond, cleared dim), a lost escort (`PICKET LOST · 2 PARTS STAY IN ITS WRECK`, the wreck ringed gold), the RECALL pill **bottom right**, disabled (`RAIDERS WITHIN 200 M`) |
| Voyage HUD B | `voyage-hud-b.png` | After the fight: salvaging (the wreck outlined gold, gold streaks and a part flying into the ship, `+ AUTOMATON CANNON`, the cargo bar ticking), `SITE CLEARED` banner, the fleet bar, the RECALL pill **under the cargo**, enabled (`WARP HOME 3 S · KEEPS THE HAUL`) |
| Ship lost | `voyage-hud-lost.png` | The last vessel lost: `SHIP LOST · RECALLED HOME`, the haul left in the wreck, `HOME IN 2 S`; the HUD dims (no variant) |
| Fleet | `fleet.png` | FLEET as a layer over home, like BLUEPRINTS: the flagship (the drydock's design), escort slots, `CORES 3 / 4`, the saved designs with their fit (`FITS · 1 CORE`, `USES BEAM LASER`, `NEEDS 2 CORES · 1 FREE`) |
| Fleet B | `fleet-b.png` | FLEET as the formation on the dock: flagship in the centre, escorts in their wing slots, an empty slot previewing the picked design, the designs as a strip below |
| Drydock | `drydock-locked.png` | The palette with a NEW tag on the freshly unlocked cannon, a gold dot on its category, a dimmed `LOCKED · BEAM LASER` row (and locked armour pills); status line `FLIGHT-READY · VOYAGE-READY · CORES 2 / 2` |
| Drydock B | `drydock-locked-b.png` | Locked parts stay hidden (GDR-0054 as it is); one dashed row `1 MORE IN RESEARCH · RESEARCH ›` instead |

The research screen shows a later moment than home (two nodes done), so the done and ready states are visible.
The not-voyage-ready wording appears in FLEET (`USES BEAM LASER`); the drydock's status line would read
`FLIGHT-READY · NOT VOYAGE-READY · USES BEAM LASER` in the same gold.

## What to decide, and the recommendation

1. **Home debrief**: a compact line at the bottom left (A) or a full debrief column after each voyage (B)?
   Recommended: **B right after a voyage, until dismissed; then A's line.** The column is where the loop's payoff
   (parts → data → unlock) is felt once; the line keeps home calm afterwards.
2. **Research**: families as columns of cards (A) or a tree with lanes and connectors (B)?
   Recommended: **A** for the slice. Nine nodes fit, every card shows cost, progress and needs without hovering,
   and it builds from `ui_kit` lists. B pays off when the trees grow towards GDR-0024's eras and cross-links.
3. **Voyage map**: a chain diagram (A) or a star map with the same chain (B)?
   Recommended: **B.** The positions are content anyway (`map: (x, y)`), it looks like the galaxy map it will
   grow into, and the glowing route keeps the order clear.
4. **RECALL pill**: bottom right (A) or under the cargo block (B)?
   Recommended: **B.** Recalling is the "bring the haul home" decision, so it sits next to the cargo it is judged
   against and next to the hull; bottom right stays free for the phone's fire and ability buttons (GDR-0008).
5. **Fleet**: a list layer like BLUEPRINTS (A) or the formation on the dock (B)?
   Recommended: **B.** It shows where escorts fly and fits the diegetic home; with up to two escorts in the slice
   the design strip is enough (it scrolls when there are more designs). A is the cheaper fallback.
6. **Locked parts in the drydock**: dimmed `LOCKED · <node>` rows (A) or hidden with a teaser row (B)?
   Recommended: **A** for the career drydock (the plan's proposal): the row names the node and its cost and
   opens it, which turns a missing part into a goal. It changes GDR-0054's "hidden" for the career, so it needs a
   record; the free drydock (`?unlock=all`) shows everything anyway.
