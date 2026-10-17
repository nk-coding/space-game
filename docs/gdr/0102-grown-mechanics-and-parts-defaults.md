# GDR-0102: The Grown's mechanics and parts, lead defaults: one regrown part every 2.5 s from a working heart, tissue that heals, a prow that deals twice and takes half, wary sites that wake on provocation and on stripped wrecks, hot stars by the inverse square

- Status: Accepted (provisional, lead), 17 Oct 2026, for Niklas to review
- Date: 2026-10-17
- Decided by: lead session (defaults within [GDR-0099](0099-the-grown-and-the-outer-reach.md), Niklas's decisions)
- Related: [GDR-0099](0099-the-grown-and-the-outer-reach.md), [GDR-0022](0022-heat-per-part.md), [GDR-0021](0021-shields.md), [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md), [GDR-0047](0047-ramming-crushes-inward-bumps-free.md), [GDR-0019](0019-damage-model.md), [GDR-0070](0070-phase-7-wave-2-defaults.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0095](0095-sensors-hidden-sites-and-scans.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0063](0063-salvage-and-research-v1.md), `docs/sim.md` "Living parts" and "Hot stars"

## Context

GDR-0099 decided what the outer reach brings (A1–A7) and left the rules and numbers open. Phase 1 of the outer reach
builds the mechanics and parts the Grown need, deterministic and tested; phase 2 builds their ships, behaviors,
encounters, cluster types, research family and bosses on top. This record holds the rules and numbers the lead
chose for phase 1. All numbers are placeholders in content.

## Options considered

- **Regrowth**: a heart that re-grows lost parts one at a time (chosen); a heart that heals every part's hit points
  (not chosen: it would make tissue armour pointless and does not "re-form blocks"); regrowth without a heart part
  (not chosen: A1 says killing the heart stops it).
- **Which parts may grow back**: only parts of the ship's own design that were destroyed (chosen); also parts that
  broke off (not chosen: a chunk drifting nearby would be duplicated).
- **Tissue armour**: an armour material that heals (chosen: hull parts take their numbers from materials, so slopes
  and blocks both come in tissue); a separate tissue block part (not chosen).
- **The ram prow**: a part that scales the ram damage when it is the touching part (chosen, on GDR-0047's rule);
  just a tough block (not chosen: a prow then only matters by its hit points, as any armour).
- **Wary**: a faction property that turns off the wake-on-sight rule for sites whose every ship is wary (chosen); an
  encounter flag (not chosen: wariness is the race's nature, GDR-0027).
- **Hot stars**: heat through every side open to space, scaled by the inverse square of the distance to a star
  beside the cluster (chosen: it reuses GDR-0022's open-side model, costs one multiplication per part); heat only
  through the sides facing the star (not chosen for now: a direction per open side for every part every tick).

## Decision

**Regrowth (A1)** — the `grown_heart` part (`heart: (interval, hp)`), 2 × 2, 6 hit points:

- A heart works while it is alive, not overheated and not idle, on its ship's controlling piece (a vessel: running,
  dark or dormant). Its timer runs only then; every **2.5 s** it re-grows one part. With nothing to re-grow the
  timer starts over, so a heart looks at most once an interval.
- What may grow back: a part of the ship's own design that was destroyed or stripped (0 hit points; a part that
  left with a split-off vessel lives on there and never grows back), never a heart; its cells free in the ship's
  grid; linked to an alive part of the heart's piece and to no alive part of another piece (a broken-off chunk is
  never grown back on; its cells wait until it is gone); and no other body overlapping its place (the physics is
  asked; the part shrunk a tenth so touching neighbours do not count). The nearest to the heart (centroid to
  centroid) goes first, ties by part index. So a ship grows back from the heart outwards, a block only from a living
  neighbour.
- A re-grown part starts with **35 %** of its hit points, cold, its shield bubble down (it recharges), and the piece
  is rebuilt in the same tick: mass, colliders, gun lines, power lists, the Rapier body (mass and shape; the velocity
  of the centre of mass follows as when a piece loses a part). `SimEvent::PartRegrown`, then `PieceChanged`.
- Killing the heart stops it; a ship without its core is salvage and its heart does nothing. Two hearts re-grow
  twice as fast. Salvage is not affected: wrecks never re-grow, and what a collector took was destroyed for the
  ship (it may grow back on a living Grown ship; the haul keeps it).

**Healing (A4)** — living parts regain hit points while they are on a vessel's controlling piece, up to their full hit
points: `grown_tissue` armour **0.04 HP/s per cell** (a full block regains its 1.1 in about 27 s; 1.6 t and 1.1 HP a
cell, build-point factor 1.3, blast factor 0.6), the ram prow **0.1 HP/s**. Salvage does not heal. A re-grown part
heals only if it is living tissue itself.

**The ram prow (A2, A4)** — `ram_prow`, 2 × 1, 5 t, 6 HP: when it is the touching part of a ram, the other side takes
**2 ×** the ram damage and its own side **0.5 ×** (content `ram: (dealt, taken)`). Everything else is GDR-0047 and
GDR-0045: the threshold, the inward crush, shields absorbing the (scaled) damage first, no damage within one owner.

**The thorn gun (A2)** — `thorn_gun`: kinetic projectiles, **1.6 damage/s at 40 m**, 4 spines a second (0.4 HP each),
110 m/s, penetration 1, recoil 0.5; 1.5 MW, heat 0.8 (limit 3). Kinetic, so a bubble takes half (GDR-0021). For
comparison the automaton cannon is 1.2/s at 75 m.

**Wary (A3)** — a faction's `wary: true` (the Grown's). A voyage site whose every ship is of a wary faction is wary
(`SitePlan::wary`): it does not wake when a player vessel comes within its wake radius. It wakes as every site does
when one of its ships is hit (a shot, a plume, a shot its bubble took, a ram with damage), when one of them stops
being a running vessel, when a scan's sweep reaches it (the scanner's wake radius, GDR-0080) and, new for every site,
when one of its wrecks is stripped (a part or its cargo collected). Automaton sites nearly always wake on sight first;
the new rule changes none of the slice's or scenarios' runs.

**Hot stars (A5)** — a cluster type's `star: (heat, distance)`: the star lies `distance` cluster radii from the centre,
in a direction rolled per cluster after its sites (so a type without a star rolls as before); every part takes
`heat × (reach / d)²` per second per side open to space, capped at **4 ×** (`data::STAR_HEAT_CAP`), where `reach` is
the star's distance from the centre and `d` the ship's (controlling piece's) distance from the star. It adds to
GDR-0022's heat model, so overheating, the 75 % restart, conduction and radiators work as before; radiators vent five
times what a plain side loses and pull heat from their neighbours. The outer reach's stub `scorched_deep` (weight 1
of 3 in the outer band, automatons standing in) has heat **0.3** at **1.8** radii: about 1.5 at the edge nearest the
star, 0.12 at the far edge. There a shield emitter open on two sides runs at about two thirds of its limit at the
centre and overheats on the star's side unless a radiator sits beside it.

**Testing and look** — the four parts and the material are locked (`?unlock=all` offers them; phase 2's research
family unlocks them); their salvage goes to the rim's nearest families (logic, hull, kinetic) until the Grown's family
exists. They use the enemy (coral) system colour like the automatons' parts; tissue's tone is `#232a1f`. In flight a
re-grown part swells softly in its hull's glow, a hot star rises as a heat-coloured glow at the view's edge towards it
(stronger nearer the star), and the voyage's subtitle says `STAR HEAT x A SIDE`; `?scene=voyage&cluster=hot` flies
the first cluster beside a hot star.

## Consequences

- Determinism: the `grown` scenario (a wary grove of two thornbacks with their wreck beside a hot star, the player's
  ships on the voyage script without its kills, a ram run every 300 ticks) is identical native against wasm; the
  six existing scenarios (core, flight, assists, behavior, salvage, voyage) run byte-identical to the build before
  (nothing they fly has a heart, tissue, a prow or a star). The content's fingerprint changed (new parts), so earlier fight codes and lab runs are
  fought under other rules.
- Phase 2 needs to know: hearts work only on the controlling piece, so a Grown ship wants its heart beside its core;
  re-growth needs a living neighbour and a free place, so a ship pressed against another grows back once they part;
  a heart overheats near a hot star (limit 4) and stops; a wary grove wakes on a stripped wreck, so its wrecks are
  bait; the outer band's cluster list and the galaxy hash change again when groves replace the stand-ins.
- Open: a heart's power or heat cost, regrowth costing materials, directional star heat, a heat shield part or heat
  research (phase 2's research family), what the HUD shows of a heart's progress.
