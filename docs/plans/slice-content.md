# Draft: the content of the first playable loop (track 7J)

Written 13 Oct 2026 by a content agent for the lead session. Status: **draft for review**. Track 7A is adding the
kinds `families`, `research`, `starts` and the `salvage` block on parts; 7E adds `voyages` and `encounters`. This
file holds the content those kinds will carry, written before the schema lands, so the field names follow the
sketches of `first-playable-loop.md` section 5.3 and may change with 7A/7E. Nothing here is in `content/` yet.

**Every number in this file is a PLACEHOLDER** (CLAUDE.md), to be tuned in track 7K with the headless voyage
runner. Texts follow the content rules: one-sentence summaries, no numbers in prose, `[[kind:id]]` links.

Read for it: `CLAUDE.md`, `docs/roadmap.md` "Phase 7", `docs/plans/first-playable-loop.md` (section 3 the slice,
5.3 the content sketches), the content and story parts of `salvage-research.md` and `galaxy-voyages.md`, GDR-0002,
0007, 0012, 0013, 0027, 0055, 0058, 0062, 0063, `docs/content-model.md`, `docs/drydock.md`, `docs/sim.md` (turning,
design report, behaviors, vessels, plumes), `docs/behavior-format.md`, all of `content/base/`, the concept board pages
01, 08 and 10 and the UI mockups (home ring, galaxy map, flight HUD).

| Section | What | Would go to |
|---|---|---|
| [1](#1-the-starter-ship-rim_skiff) | The starter ship `rim_skiff` | `content/base/ships/rim_skiff.ron` |
| [2](#2-two-new-enemies) | `automaton_hunter`, `automaton_warden` | `content/base/ships/automaton_hunter.ron`, `automaton_warden.ron` |
| [3](#3-voyages-and-encounters) | Four voyages, the encounters | `content/base/voyages/*.ron`, `content/base/encounters/automatons.ron` |
| [4](#4-research-families-nodes-salvage-values-locks) | Families, nine nodes, salvage values, locks | `families.ron`, `research/rim.ron`, `salvage` blocks, `locked` flags, `materials/precursor.ron` |
| [5](#5-texts) | The start, debrief and milestone lines, lore | `starts.ron`, `voyages/*.ron`; UI strings in the client |
| [6](#6-the-escort-behavior) | The `escort` behavior | `content/base/behaviors/escort.ron` |
| [7](#7-open-questions-for-the-lead) | Open content questions | – |

**How the ship numbers were checked.** Sections 1 and 2 give the arithmetic by hand from the part numbers and the
formulas of `docs/sim.md`. To be sure the hand rules are the sim's, a throwaway script (not in the repo, ADR-0025)
applied the same rules (mass and polygon inertia, links and exposure, radiator and capacitor bonuses, the power
pass, the steady heat solve, the report's turn loop with the last-resort gate, the rated-speed falloff) to the four
base ships first: it reproduces the design report's published numbers for all of them (raider 15.8 t, 16.9 t·m²,
42 % heat, 0.567 s; Runner 106.5 t, 1176 t·m², 109 % heat, 1.50 s; sample early 43 t, 214 t·m², 82 %, 1.60 s;
sample mid 88 t, 1010 t·m², 78 %, 3.40 s). The lead should still run `sim::design_report` on the real files.

---

## 1. The starter ship `rim_skiff`

The concept board's "early game" ship (`10-art-direction.png`: steel, fission, pulse lasers, thrusters; the same
layout as `sample_early`), made flight-ready under GDR-0055: an RCS pod on each wing tip, outboard of the radiators,
with its front, back and outer sides open. The radiators lost the slopes under them, so they vent above and below.
Early-game kit only (GDR-0063): steel, slopes, fission reactors, capacitors, radiators, pulse lasers, shield
emitters, thrusters, RCS pods, logic cores, cargo bays. Frigate frame (10×4 of 24×24), two logic cores (mind
capacity 2 = this one ship).

```
       0   1   2   3   4   5   6   7   8   9          forward = up
  0    .   .   .   /   L   L   \   .   .   .          L pulse laser     C capacitor      A steel armour block
  1    .   .   /   A   C   C   A   \   .   .          P RCS pod         R radiator       F fission reactor
  2    P   R   F   B   K   K   B   F   R   P          B cargo bay       K logic core     S shield emitter
  3    .   .   T   S   A   A   S   T   .   .          T thruster (exhaust down, off the grid)   / \ steel slopes
```

```ron
// Rim skiff: the starter ship of a new game (`starts.ron`: rim_awakening). The concept board's "early game" ship
// (docs/concept/boards/concept-board/10-art-direction.png, as sample_early) made flight-ready under GDR-0055: an RCS
// pod on each wing tip turns it (front, back and outer side open), and the radiators vent above and below instead
// of into slopes. Early-game kit only (GDR-0063). PLACEHOLDER layout until the design report and the voyage runner
// have checked it (track 7K).
(
    id: "rim_skiff",
    name: "Rim skiff",
    summary: "A small steel ship with twin pulse lasers, two fission reactors and RCS pods on its wing tips.",
    description: "The one design the shipyard still remembered. Two [[part:cargo_bay|cargo bays]] bring home what the wrecks give up, and the [[part:rcs_pod|RCS pods]] on its wing tips turn it quickly.",
    faction: "ship_mind",
    size: (10, 4),
    parts: [
        (part: "slope_1x1", x: 3, y: 0, flip: true, material: "steel"),
        (part: "pulse_laser", x: 4, y: 0),
        (part: "pulse_laser", x: 5, y: 0),
        (part: "slope_1x1", x: 6, y: 0, material: "steel"),
        (part: "slope_1x1", x: 2, y: 1, flip: true, material: "steel"),
        (part: "armor_block", x: 3, y: 1, material: "steel"),
        (part: "capacitor", x: 4, y: 1),
        (part: "capacitor", x: 5, y: 1),
        (part: "armor_block", x: 6, y: 1, material: "steel"),
        (part: "slope_1x1", x: 7, y: 1, material: "steel"),
        (part: "rcs_pod", x: 0, y: 2),
        (part: "radiator", x: 1, y: 2),
        (part: "fission_reactor", x: 2, y: 2),
        (part: "cargo_bay", x: 3, y: 2),
        (part: "logic_core", x: 4, y: 2),
        (part: "logic_core", x: 5, y: 2),
        (part: "cargo_bay", x: 6, y: 2),
        (part: "fission_reactor", x: 7, y: 2),
        (part: "radiator", x: 8, y: 2),
        (part: "rcs_pod", x: 9, y: 2),
        (part: "thruster", x: 2, y: 3),
        (part: "shield_emitter", x: 3, y: 3),
        (part: "armor_block", x: 4, y: 3, material: "steel"),
        (part: "armor_block", x: 5, y: 3, material: "steel"),
        (part: "shield_emitter", x: 6, y: 3),
        (part: "thruster", x: 7, y: 3),
    ],
)
```

### 1.1 Estimated numbers (hand arithmetic, content of 13 Oct)

| Number | Value | Arithmetic |
|---|---|---|
| Mass | **43 t** | slopes 4 × 1 (steel 2 t/cell × ½) + armour 4 × 2 + lasers 2 × 2 + capacitors 2 × 1 + pods 2 × 1 + radiators 2 × 1 + reactors 2 × 4 + bays 2 × 1.5 + cores 2 × 1 + thrusters 2 × 2 + shields 2 × 2 = 4 + 8 + 4 + 2 + 2 + 2 + 8 + 3 + 2 + 4 + 4 |
| Centre of mass | (5.0, 2.33) | mirror-symmetric, so x = 5.0; y = Σ m·y / M = (6 t × 0.56 + 8 t × 1.54 + 17 t × 2.5 + 12 t × 3.5) / 43 = 100.2 / 43 |
| Inertia | **≈ 230 t·m²** | Σ m·r² ≈ 223 (biggest terms: reactors 2 × 25.1, pods 2 × 20.3, thrusters 2 × 15.2, radiators 2 × 12.3) + each part's own (squares m/6, slopes m/9) ≈ 7 |
| Turning torque, first line | 1900 kN·m | cores 2 × 50 = 100 pure; RCS couple: the right pod's front nozzle and the left pod's back nozzle, 200 kN each at 4.5 m from the centre line: 2 × 200 × 4.5 = 1800 |
| Planning α | 8.3 rad/s² | 1900 / 230 |
| **Turn 90°** | **≈ 0.8 s** | the report's heading loop (ω = min(√(2 · 0.7 · α · e), 12 · e, 6)) gives 0.83 s on the first line, ≈ 2.39 / √α (this closed form hits the raider's 0.57 s and the Runner-with-pods 1.17 s / 0.97 s); the last resort (differential thrust of the thrusters, 200 kN × 2.5 m = 500 kN·m) joins after 0.2 s: ≈ 0.77 s. Between the raider (0.57 s) and the Runner (1.50 s), as "small ships snappier" asks |
| Keyboard turn | 1.5 rad/s, the full key rate | min(1.5, √(0.7 · 8.3 · π) = 4.3) (sample early only gets 1.0) |
| Thrust forward | 800 kN → **18.6 m/s²** | thrusters 2 × 200 + the pods' back nozzles 2 × 200; the pods fade from 11.2 m/s (80 % of 14) to 0 at 14 m/s, so above that 400 kN → 9.3 m/s² |
| **Top speed** | **20 m/s** | the thrusters' rated speed (GDR-0041, GDR-0055) |
| 0 → 95 % top speed | ≈ 1.6 s | 0 → 11.2 m/s at 18.6 m/s² (0.60 s) + 11.2 → 14 (pods fading, ≈ 0.20 s) + 14 → 16 at 9.3 (0.22 s) + 16 → 19 fading (4 · 43 / 400 · ln 4 = 0.60 s) |
| Backward / sideways | 400 kN (9.3 m/s²) / 200 kN (4.7 m/s²), 14 m/s | the pods' front nozzles; one outer nozzle each way. It can back off nose-on (`kite`) and strafe under a lock |
| **Firepower** | **2.3 /s**, reach 60 m | 2 pulse lasers × 1 /s × 1.15 (one linked capacitor each, +15 %) |
| Power | 24 / 22 MW | supply 2 × (10 + 2 from the venting radiator beside each reactor); demand at full use: lasers 8 + shields 6 + thrusters 4 + pods 2 + cores 2 |
| Peak heat (full use) | **83 %** (the pulse lasers, 2.49 of 3) | reactor heat 3 × (⅓ + ⅔ × 22/24) = 2.83; each radiator vents two open sides, 2 × 1.5; sample early runs at 82 % |
| Control | 10 / 32 | lasers, thrusters, shields, pods, reactors: 1 each; two cores × 16 |
| Rule slots, cargo, hit points, build points | 12, **8 units**, 24 HP, 53 BP | two cores × 6; two cargo bays × 4 (the flight HUD mockup's `3 / 8`) |
| Device load | ≈ 0.8 µs (Light) | the report's cost model: 26 parts, 10 thruster rows and torque sources, 2 guns, 1 piece, about 10 collider shapes |

Flight-ready under the report's rules: one piece; both cores powered first (`Critical`); thrust; both guns clear (top
row); both thruster plumes leave the grid (bottom row, nothing behind); both radiators vent (top and bottom open);
each pod has three nozzles (front, back, outer); control well under capacity; mirror-symmetric, so the yaw is
balanced. No warnings expected.

Notes for 7K:

- **Its weak side is deliberate**: the raiders' cannons reach 75 m, its pulse lasers 60 m, and `raider_basic` stands
  off at 64 m, so the player has to close in. That is the hook of the first research (the reverse-engineered cannon).
- It is fragile (24 HP, like sample early's 23) and has no repairs on a voyage. If the runner shows that Shipyard
  approach (2 + 2 + 3 raiders) can't be cleared in one run, the levers in order: encounter counts (section 3),
  then an armed variant (pulse lasers on the wing shoulders in place of the row-1 slopes, capacitors beside them:
  4.6 /s, but it needs a third reactor: demand 30 MW against 24).
- The pods make it a quick starter at low speed (18.6 m/s² below 11 m/s). That is GDR-0058's open point (RCS nozzle
  200 kN vs 150 kN); with 150 kN: a 1350 kN·m couple, ≈ 0.95 s per 90°, 16.3 m/s².

---

## 2. Two new enemies

Both use the automatons' look: automaton plate with slope corners and coral glow, the raider's parts (automaton
cores, automaton cannons, the thruster) and, on the Warden, shield emitters. A raider's only core is also its power
(6 MW, GDR-0063): an automaton ship becomes salvage only when its last core dies, and every destroyed core leaves a
programmable-matter fragment. The designs below take that into account: one automaton core powers exactly two
cannons and one thruster (2 + 2 + 2 = 6 MW) and runs three of its four control.

### 2.1 `automaton_hunter`

A wider, heavier raider with a cannon on each forward prong and its core buried in the plating. The prongs are
1-HP cannons linked only through their back side: aim at `weapons` to disarm it, or at the core to take both
cannons home.

```
       0   1   2   3   4
  0    .   C   .   C   .          C automaton cannon    K automaton core    T thruster
  1    /   A   A   A   \          A automaton plate     / \ plate slopes
  2    \   A   K   A   /
  3    .   \   T   /   .
```

```ron
// Automaton hunter: a heavier raider that leads the packs deeper in the rim (the slice's Corran Reach). Two
// cannons on forward prongs, the core buried in the middle. One automaton core powers exactly two cannons and one
// thruster (6 MW) and runs three of its four control. PLACEHOLDER layout (track 7J, 13 Oct 2026).
(
    id: "automaton_hunter",
    name: "Automaton hunter",
    summary: "A 5×4 rogue automaton with a cannon on each forward prong.",
    description: "Hunters lead the packs deeper in the rim. The [[part:automaton_core|core]] sits deep in the plating: shoot the prongs to disarm one, or the core to keep both cannons for salvage.",
    tags: ["enemy"],
    faction: "automatons",
    size: (5, 4),
    parts: [
        (part: "automaton_cannon", x: 1, y: 0),
        (part: "automaton_cannon", x: 3, y: 0),
        (part: "slope_1x1", x: 0, y: 1, flip: true, material: "automaton_plate"),
        (part: "armor_block", x: 1, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 2, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 3, y: 1, material: "automaton_plate"),
        (part: "slope_1x1", x: 4, y: 1, material: "automaton_plate"),
        (part: "slope_1x1", x: 0, y: 2, rot: 2, material: "automaton_plate"),
        (part: "armor_block", x: 1, y: 2, material: "automaton_plate"),
        (part: "automaton_core", x: 2, y: 2),
        (part: "armor_block", x: 3, y: 2, material: "automaton_plate"),
        (part: "slope_1x1", x: 4, y: 2, rot: 2, flip: true, material: "automaton_plate"),
        (part: "slope_1x1", x: 1, y: 3, rot: 2, material: "automaton_plate"),
        (part: "thruster", x: 2, y: 3),
        (part: "slope_1x1", x: 3, y: 3, rot: 2, flip: true, material: "automaton_plate"),
    ],
)
```

| | Hunter | (raider, for comparison) |
|---|---|---|
| Mass | 26.6 t (cannons 4 + core 3 + thruster 2 + 5 plates × 2.2 + 6 slopes × 1.1) | 15.8 t |
| Inertia | 54.6 t·m² | 17 |
| Turn 90° | **1.03 s** (core 300 kN·m / 54.6 = 5.5 rad/s²; no last resort: one engine on the centre line) | 0.57 s |
| Thrust, top speed, 0 → 95 % | 200 kN → 7.5 m/s², 20 m/s, 2.9 s | 12.7 m/s², 20 m/s, 1.7 s |
| Firepower | **2.4 /s** (2 cannons × 1.2), reach 75 m | 1.2 /s |
| Power, control, heat | 6 / 6 MW, 3 / 4, peak 35 % (the thruster) | 6 / 4, 2 / 4, 42 % |
| Hit points, build points, rule slots | 14.6 HP, 23.6 BP, 2 (`raider_basic` fits) | 8.8 HP, 15.8 BP, 2 |
| Salvage when its core dies | 2 cannons (kinetic 3 each), a thruster, 5 plate blocks and 6 slopes, 1 fragment | 1 cannon, a thruster, 6 plate parts, 1 fragment |

It is the slow, hard-hitting one of the pair: twice the guns and the armour of a raider, but it turns and
accelerates like a much bigger ship. A second thruster would make it a real hunter (13 m/s²) but needs 8 MW from a
6 MW core (open question 3).

### 2.2 `automaton_warden` (the slice's boss)

A shielded **heart** (core, two shield emitters, heavy plate) between two **fists** (each: a core, two cannons, a
thruster). Each fist hangs on the heart by one bridge block (`=`), shielded from the front by a slope. The three
cores split the power exactly: 3 × 6 = 18 MW = four cannons 8 + two shields 6 + two thrusters 4, and the only split
of the draw into three self-sufficient 6 MW sections is {shield, shield}, {cannon, cannon, thruster} ×2. So when a
bridge breaks, that fist flies on as a hunter-like vessel of its own (GDR-0013), and the heart stays behind as an
immobile, unarmed vessel that holds its shields up until its core dies.

```
       0   1   2   3   4   5   6   7   8
  0    C   C   \   /   A   \   /   C   C          C automaton cannon   K automaton core   S shield emitter
  1    A   A   =   A   A   A   =   A   A          T thruster           A automaton plate  = bridge (plate)
  2    A   K   .   A   A   A   .   K   A          / \ plate slopes
  3    A   A   .   A   S   A   .   A   A          left fist: x 0–2 rows 0–4 (with the slope at 2,0)
  4    \   T   .   A   K   A   .   T   /          heart: x 3–5 rows 0–6
  5    .   .   .   A   S   A   .   .   .          right fist: x 6–8 rows 0–4
  6    .   .   .   \   A   /   .   .   .
```

The heart's bubbles (radius 2.3 m around (4.5, 3.5) and (4.5, 5.5)) cover the heart and its core twice over, but
not the bridges (2.8 m away) and not the fists: the fists are bare, the bridges can be shot from behind, and the
heart is the last, shielded stand.

```ron
// Automaton warden: the guardian at the end of the slice (voyage wardens_gate). A shielded heart between two armed
// fists, each section with its own automaton core and its own 6 MW (heart: two shield emitters; each fist: two
// cannons and a thruster), joined by one bridge block per fist. Shot apart, the fists fly on as vessels of their
// own (GDR-0013) and the heart keeps its shields up until its core dies. PLACEHOLDER layout (track 7J, 13 Oct 2026).
(
    id: "automaton_warden",
    name: "Automaton warden",
    summary: "A rogue automaton guardian: a shielded heart between two armed fists.",
    description: "Three [[part:automaton_core|cores]] run it, one in the heart and one in each fist. Break a bridge and that fist fights on alone; the heart holds its [[part:shield_emitter|shields]] up until its core goes dark.",
    lore: "It was built to keep the gate shut until its makers came back.",
    tags: ["enemy", "boss"],
    faction: "automatons",
    size: (9, 7),
    parts: [
        (part: "automaton_cannon", x: 0, y: 0),
        (part: "automaton_cannon", x: 1, y: 0),
        (part: "slope_1x1", x: 2, y: 0, material: "automaton_plate"),
        (part: "slope_1x1", x: 3, y: 0, flip: true, material: "automaton_plate"),
        (part: "armor_block", x: 4, y: 0, material: "automaton_plate"),
        (part: "slope_1x1", x: 5, y: 0, material: "automaton_plate"),
        (part: "slope_1x1", x: 6, y: 0, flip: true, material: "automaton_plate"),
        (part: "automaton_cannon", x: 7, y: 0),
        (part: "automaton_cannon", x: 8, y: 0),
        (part: "armor_block", x: 0, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 1, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 2, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 3, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 4, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 5, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 6, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 7, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 8, y: 1, material: "automaton_plate"),
        (part: "armor_block", x: 0, y: 2, material: "automaton_plate"),
        (part: "automaton_core", x: 1, y: 2),
        (part: "armor_block", x: 3, y: 2, material: "automaton_plate"),
        (part: "armor_block", x: 4, y: 2, material: "automaton_plate"),
        (part: "armor_block", x: 5, y: 2, material: "automaton_plate"),
        (part: "automaton_core", x: 7, y: 2),
        (part: "armor_block", x: 8, y: 2, material: "automaton_plate"),
        (part: "armor_block", x: 0, y: 3, material: "automaton_plate"),
        (part: "armor_block", x: 1, y: 3, material: "automaton_plate"),
        (part: "armor_block", x: 3, y: 3, material: "automaton_plate"),
        (part: "shield_emitter", x: 4, y: 3),
        (part: "armor_block", x: 5, y: 3, material: "automaton_plate"),
        (part: "armor_block", x: 7, y: 3, material: "automaton_plate"),
        (part: "armor_block", x: 8, y: 3, material: "automaton_plate"),
        (part: "slope_1x1", x: 0, y: 4, rot: 2, material: "automaton_plate"),
        (part: "thruster", x: 1, y: 4),
        (part: "armor_block", x: 3, y: 4, material: "automaton_plate"),
        (part: "automaton_core", x: 4, y: 4),
        (part: "armor_block", x: 5, y: 4, material: "automaton_plate"),
        (part: "thruster", x: 7, y: 4),
        (part: "slope_1x1", x: 8, y: 4, rot: 2, flip: true, material: "automaton_plate"),
        (part: "armor_block", x: 3, y: 5, material: "automaton_plate"),
        (part: "shield_emitter", x: 4, y: 5),
        (part: "armor_block", x: 5, y: 5, material: "automaton_plate"),
        (part: "slope_1x1", x: 3, y: 6, rot: 2, material: "automaton_plate"),
        (part: "armor_block", x: 4, y: 6, material: "automaton_plate"),
        (part: "slope_1x1", x: 5, y: 6, rot: 2, flip: true, material: "automaton_plate"),
    ],
)
```

| | Whole Warden | A fist after its bridge breaks | The heart alone |
|---|---|---|---|
| Parts, mass | 45 parts, **91 t** (fists 2 × 22.2 + bridges 2 × 2.2 + heart 42.2) | 11, 22.2 t | 21, 42.2 t |
| Inertia | 839 t·m² | 49.8 t·m² | – |
| Turn 90° | **≈ 1.9 s** (cores 3 × 300 = 900 kN·m → α 1.07 rad/s², 2.4 s on the first line; the fists' thrusters 3 m off the centre line add 600 kN·m after 0.2 s) | 0.97 s | turns only (core torque), no thrust |
| Thrust, top speed, 0 → 95 % | 400 kN → 4.4 m/s², 20 m/s, 4.9 s | 9.0 m/s², 20 m/s, 2.4 s | none |
| Firepower | **4.8 /s** (4 cannons), reach 75 m | 2.4 /s | none |
| Shields | 2 bubbles: capacity 6 each, regen 1.5 /s each, energy fully absorbed, kinetic half | – | the same 2 bubbles |
| Power, control, heat | 18 / 18 MW, 8 / 12, peak 40 % (cannons) | 6 / 6, 3 / 4, 38 % | 6 / 6, 2 / 4 |
| Hit points, build points, rule slots | 50 HP, 75 BP, 6 | 12.2 HP, 2 | 2 |
| Fragments | 3 cores → up to 3 programmable-matter fragments | | |

Behavior: the faction's `raider_basic` (two rules, so each piece keeps all of it after a split, GDR-0035). It turns
slowly, so a quick player can circle it; the raiders it comes with (section 3) are there to stop that. Its grid is
9×7 rather than 9×9: two more heart rows would add about 13 t and slow it further (open question 4).

---

## 3. Voyages and encounters

### 3.1 The slice's numbers (from `first-playable-loop.md` section 3, all PLACEHOLDERS)

| What | Value | Where it lives |
|---|---|---|
| Voyages | 4 in a chain: Shipyard approach → Halvex Drift → Corran Reach → The Warden's gate | content (`opens_after`) |
| Radius | 600 / 900 / 1000 / 900 m | content (`radius`) |
| Site spacing | 400–700 m (≈ 20–35 s at 20–22 m/s) | content (`spacing`) |
| Sites | 4 / 5 / 5 / 4, one goal each | content (`sites`) |
| Wake radius | 250 m for packs (the plan), 200 m for wrecks sites (the plan), 300 m for dens and the Warden (new) | content (`wake_radius`) |
| Spawn radius | 600 m: a site's ships and wrecks spawn when a player vessel comes this close | `SimConfig` (7E), not content |
| Recall | no awake enemy vessel within 200 m; 3 s warp-out | `SimConfig` (7E/7H) |
| Salvage | reach 5 m, 3 parts per second, best value per cargo unit first | `SimConfig::salvage` (7B) |
| Derelict damage share | precursor wrecks 0.4 (the plan); derelict field 0.5, core graveyard 0.3 (new) | content (`wrecks[].damage`) |
| Expected length | 6–8 / 10–15 / 12–15 / 10–20 min | content (`minutes`, a proposed field for the map's `≈ 8 MIN`) |

### 3.2 Encounters

Five kinds, as the plan's table (raider pack/den, hunter pack/den, derelict field, precursor wrecks, core
graveyard), plus the Warden. The variants by count (a pair, a pack, a den) are separate entries, which needs no
schema beyond 5.3; a per-site count override is the alternative (open question 5).

```ron
// encounters/automatons.ron: what waits at a voyage site. Ships hold until a player vessel comes within
// `wake_radius`, then fly their faction's behavior; wrecks spawn as salvage with `damage` (a share of their parts)
// destroyed (rolled on the sim's encounter stream). PLACEHOLDER NUMBERS (track 7J, 13 Oct 2026; tuned in 7K).
[
    (
        id: "raider_pair",
        name: "Raider pair",
        summary: "Two raiders picking over the wreckage, waking when a ship comes near.",
        tags: ["raiders"],
        ships: [(ship: "automaton_raider", count: (2, 2))],
        wake_radius: 250,
    ),
    (
        id: "raider_pack",
        name: "Raider pack",
        summary: "A few raiders drifting among the rocks, waking when a ship comes near.",
        tags: ["raiders"],
        ships: [(ship: "automaton_raider", count: (3, 3))],
        wake_radius: 250,
    ),
    (
        id: "raider_den",
        name: "Raider den",
        summary: "Where the raiders gather: a big pack that wakes all at once.",
        tags: ["raiders"],
        ships: [(ship: "automaton_raider", count: (4, 5))],
        wake_radius: 300,
    ),
    (
        id: "hunter_pack",
        name: "Hunter pack",
        summary: "Heavier automatons with a cannon on each prong, and a raider running ahead of them.",
        tags: ["hunters"],
        ships: [
            (ship: "automaton_hunter", count: (2, 2)),
            (ship: "automaton_raider", count: (0, 1)),
        ],
        wake_radius: 250,
    ),
    (
        id: "hunter_den",
        name: "Hunter den",
        summary: "The hunters' lair, guarded by the whole pack.",
        tags: ["hunters"],
        ships: [
            (ship: "automaton_hunter", count: (2, 3)),
            (ship: "automaton_raider", count: (1, 2)),
        ],
        wake_radius: 300,
    ),
    (
        id: "derelict_field",
        name: "Derelict field",
        summary: "Old shipyard hulls, long dead, with a raider or two picking at them.",
        lore: "Hulls from the shipyard's last launch. Not one of them got past the rim.",
        tags: ["wrecks"],
        ships: [(ship: "automaton_raider", count: (0, 1))],
        wrecks: [
            (ship: "sample_early", count: (1, 1), damage: 0.5),
            (ship: "automaton_raider", count: (2, 3), damage: 0.5),
        ],
        wake_radius: 200,
    ),
    (
        id: "precursor_wrecks",
        name: "Precursor wrecks",
        summary: "Old hulls full of parts the shipyard has forgotten how to make.",
        lore: "Their logs stop mid-sentence, all on the same day.",
        tags: ["wrecks"],
        ships: [(ship: "automaton_raider", count: (0, 2))],
        wrecks: [(ship: "sample_mid", count: (1, 2), damage: 0.4)],
        wake_radius: 200,
    ),
    (
        id: "core_graveyard",
        name: "Core graveyard",
        summary: "Burned-out raiders lying in rows, many of their cores still whole.",
        lore: "Raiders drag their burned-out kin here and leave them in rows. Nobody taught them to.",
        tags: ["wrecks", "cores"],
        ships: [(ship: "automaton_raider", count: (1, 2))],
        wrecks: [(ship: "automaton_raider", count: (4, 6), damage: 0.3)],
        wake_radius: 200,
    ),
    (
        id: "the_warden",
        name: "The Warden",
        summary: "The gate's old guardian, with raiders at its side.",
        description: "Shoot the [[ship:automaton_warden]] apart and it fights on in pieces: each fist flies on alone, and the shielded heart holds until its core goes dark.",
        lore: "Nobody ever told it that its makers are not coming back.",
        tags: ["boss"],
        ships: [
            (ship: "automaton_warden", count: (1, 1)),
            (ship: "automaton_raider", count: (2, 4)),
        ],
        wake_radius: 300,
    ),
]
```

What each gives (with the salvage values of section 4.3):

| Encounter | Fight | Salvage, mostly |
|---|---|---|
| Raider pair / pack / den | 2 / 3 / 4–5 raiders | kinetic (cannons), propulsion (thrusters), hull (plate), a fragment per kill |
| Hunter pack / den | 2 hunters (+0–1 raider) / 2–3 hunters + 1–2 raiders | twice the kinetic per kill |
| Derelict field | 0–1 raider | steel, pulse lasers, fission reactors, capacitors, maybe a logic core: a first taste of salvage without a fight |
| Precursor wrecks | 0–2 raiders | composite (hull), beams (energy), fusion (power), torch drive (propulsion), logic cores |
| Core graveyard | 1–2 raiders | intact automaton cores: logic data and programmable matter (with damage 0.3 about 2 in 3 cores survive) |
| The Warden | the Warden + 2–4 raiders | cannons, shield emitters, three fragments; the end of the slice |

### 3.3 Voyages

Map positions follow the galaxy mockup's look (the shipyard bottom left, the chain running out towards the band's
edge); the slice map is its own small map, so they are only places on it.

```ron
// voyages/yard_approach.ron. PLACEHOLDER NUMBERS (track 7J, 13 Oct 2026; tuned in 7K).
(
    id: "yard_approach",
    name: "Shipyard approach",
    summary: "The quiet lanes outside the shipyard, where a few raiders pick over old wrecks.",
    band: "rim",
    map: (0.28, 0.66),
    radius: 600,
    spacing: (400, 700),
    minutes: (6, 8),                     // proposed field: the map's "≈ 8 MIN"
    cleared: "The lanes around the shipyard are quiet again. Further out, the Halvex Drift is waiting.",   // proposed
    sites: [
        (encounter: "raider_pair", count: 2),
        (encounter: "derelict_field"),
        (encounter: "raider_pack", goal: true),
    ],
)
```

```ron
// voyages/halvex_drift.ron. PLACEHOLDER NUMBERS (track 7J, 13 Oct 2026; tuned in 7K).
(
    id: "halvex_drift",
    name: "Halvex Drift",
    summary: "A raider-haunted drift of wrecks one warp out from the shipyard.",
    band: "rim",
    map: (0.44, 0.30),
    opens_after: ["yard_approach"],
    radius: 900,
    spacing: (400, 700),
    minutes: (10, 15),
    cleared: "The raiders' den is broken. Their hunters still hold the Corran Reach.",
    sites: [
        (encounter: "raider_pack", count: 2),
        (encounter: "precursor_wrecks"),
        (encounter: "core_graveyard"),
        (encounter: "raider_den", goal: true),
    ],
)
```

```ron
// voyages/corran_reach.ron. PLACEHOLDER NUMBERS (track 7J, 13 Oct 2026; tuned in 7K).
(
    id: "corran_reach",
    name: "Corran Reach",
    summary: "A long reach of broken hulls where heavier automatons hunt in packs.",
    band: "rim",
    map: (0.62, 0.52),
    opens_after: ["halvex_drift"],
    radius: 1000,
    spacing: (400, 700),
    minutes: (12, 15),
    cleared: "The hunters are scattered. Beyond the Reach, something old still keeps the gate.",
    sites: [
        (encounter: "hunter_pack", count: 2),
        (encounter: "raider_pack"),
        (encounter: "precursor_wrecks"),
        (encounter: "hunter_den", goal: true),
    ],
)
```

```ron
// voyages/wardens_gate.ron. PLACEHOLDER NUMBERS (track 7J, 13 Oct 2026; tuned in 7K).
(
    id: "wardens_gate",
    name: "The Warden's gate",
    summary: "The gate to the outer reach, and the old guardian that still keeps it.",
    band: "rim",
    map: (0.80, 0.34),
    opens_after: ["corran_reach"],
    radius: 900,
    spacing: (400, 700),
    minutes: (10, 20),
    cleared: "The Warden goes dark. The gate behind it stays cold: whatever opened it once is gone.",
    sites: [
        (encounter: "raider_pack", count: 2),
        (encounter: "core_graveyard"),
        (encounter: "the_warden", goal: true),
    ],
)
```

Changes against the plan's table (3.3): Halvex Drift's packs are 3 raiders instead of 3–4 (one `raider_pack`
entry serves voyages 1, 2 and 4; voyage 1's goal must stay gentle), and Corran Reach's lone raider pack is a pack
of 3. Both are placeholders for 7K.

---

## 4. Research: families, nodes, salvage values, locks

### 4.1 Families (`families.ron`)

GDR-0024's trees. Missiles wait for their first part; a family without nodes in the slice still lists, so salvage
data and saves never need a migration when its tree arrives.

```ron
// families.ron: the tech families of GDR-0024, one research tree each. Order = display order (the research
// screen's columns, the haul line). `color` is a system colour key (art/tokens.json ship.system_colors).
// Missiles join with their first part.
[
    (id: "hull", name: "Hull and armour", summary: "Plating, slopes and the drydock frames that hold a ship together."),
    (id: "power", name: "Power", summary: "Reactors and capacitors that feed each piece's shared power pool.", color: "power"),
    (id: "energy", name: "Energy weapons", summary: "Lasers and beams: fire that hits at once and burns the first block it touches.", color: "weapons"),
    (id: "kinetic", name: "Kinetic weapons", summary: "Guns that throw mass; shields stop only part of each shot.", color: "weapons"),
    (id: "shields", name: "Shields", summary: "Bubbles that soak up hits before they reach the hull.", color: "shields"),
    (id: "thermal", name: "Thermal", summary: "Radiators and everything else that keeps a ship from cooking itself.", color: "heat"),
    (id: "propulsion", name: "Propulsion", summary: "Engines, gyros and RCS pods: everything that pushes and turns a ship.", color: "propulsion"),
    (id: "logic", name: "Logic", summary: "Cores that host the ship-mind, and room in the mind to run more of them.", color: "logic"),
    (id: "utility", name: "Utility", summary: "Cargo, salvage gear and drones.", color: "cargo"),
]
```

The plan's sketch gave hull `color: "steel"`, which is an armour tone, not a system colour key; hull has no colour
here, like the hull category.

### 4.2 The nine nodes (`research/rim.ron`)

```ron
// research/rim.ron: the slice's nodes (first-playable-loop.md 3.2). PLACEHOLDER COSTS (track 7J, 13 Oct 2026; tuned
// in 7K). Kinds per GDR-0024. Unlocking is instant and permanent; unlocked parts are unlimited (GDR-0063).
[
    (
        id: "automaton_cannon",
        name: "Reverse-engineered cannon",
        summary: "Build the raiders' crude kinetic gun into your own ships.",
        description: "Shields stop only part of each kinetic shot, and slopes hit at a shallow angle glance it off.",
        family: "kinetic",
        kind: NewPart,
        cost: (data: {"kinetic": 12}),
        unlocks: [Part("automaton_cannon")],
    ),
    (
        id: "composite_plating",
        name: "Composite plating",
        summary: "Press the precursor hulls' plating anew: lighter and tougher than steel.",
        family: "hull",
        kind: NewPart,
        cost: (data: {"hull": 20}),
        unlocks: [Material("composite")],
    ),
    (
        id: "beam_laser",
        name: "Beam laser",
        summary: "Copy the old hulls' continuous beam: strong, hungry and hot.",
        family: "energy",
        kind: NewPart,
        cost: (data: {"energy": 18}),
        unlocks: [Part("beam_laser")],
    ),
    (
        id: "fusion_reactor",
        name: "Fusion reactor",
        summary: "Rebuild the precursors' big reactor: plenty of power, plenty of heat.",
        family: "power",
        kind: NewPart,
        cost: (data: {"power": 20}),
        unlocks: [Part("fusion_reactor")],
    ),
    (
        id: "torch_drive",
        name: "Torch drive",
        summary: "Rebuild the precursors' main drive: hot, heavy and very fast.",
        family: "propulsion",
        kind: NewPart,
        cost: (data: {"propulsion": 20}),
        unlocks: [Part("torch_drive")],
    ),
    (
        id: "tamed_core",
        name: "Tamed automaton core",
        summary: "Burn the feral mind out of a raider's core and keep the shell: a small core for drones.",
        description: "An [[part:automaton_core]] has few rule slots and runs few parts, but it powers itself. Each one still counts against your mind capacity.",
        family: "logic",
        kind: NewPart,
        cost: (data: {"logic": 8}, materials: {"programmable_matter": 2}),   // plan: 1 (open question 7)
        unlocks: [Part("automaton_core")],
    ),
    (
        id: "mind_expansion_1",
        name: "Mind expansion I",
        summary: "Run one more logic core at once: room for an escort or a drone.",
        family: "logic",
        kind: Refinement,
        cost: (data: {"logic": 10}, materials: {"programmable_matter": 5}),  // plan: 2
        requires: ["tamed_core"],
        unlocks: [MindCapacity(1)],
    ),
    (
        id: "mind_expansion_2",
        name: "Mind expansion II",
        summary: "Stretch the mind over one more core.",
        family: "logic",
        kind: Refinement,
        cost: (data: {"logic": 20}, materials: {"programmable_matter": 10}), // plan: 4
        requires: ["mind_expansion_1"],
        unlocks: [MindCapacity(1)],
    ),
    (
        id: "cruiser_drydock",
        name: "Cruiser drydock",
        summary: "Expand the drydock for line ships with several big machines.",
        family: "hull",
        kind: SizeClass,
        cost: (data: {"hull": 30}),
        requires: ["composite_plating"],
        unlocks: [Frame("cruiser")],
    ),
]
```

Data and programmable matter are raised against the plan only where GDR-0063 changed the supply: every destroyed core
now leaves a fragment, so a first voyage alone (seven raiders) could pay the plan's 1 + 2 + 4 (open question 7).

### 4.3 Salvage values (the `salvage` block on parts and armour materials)

From the plan's 3.2, completed for every part a slice wreck can hold. A part takes cargo units equal to its
footprint cells (code rule). "Per unit" is what the strip order sorts by.

| Part or material | Family, data | Materials | Per unit | Notes |
|---|---|---|---|---|
| `automaton_core` (intact) | logic 4 | programmable matter 1 | 4 + PM | graveyards and derelicts only |
| `logic_core` (intact) | logic 3 | programmable matter 1 | 3 + PM | derelicts |
| destroyed core: fragment | – | programmable matter 1 | PM | GDR-0063; 1 unit; field name for 7A (`fragment: {...}`?) |
| `automaton_cannon` | kinetic 3 | | 3 | |
| `pulse_laser` | energy 3 | | 3 | |
| `beam_laser` | energy 7 | | 3.5 | 2 units |
| `fission_reactor` | power 4 | | 4 | |
| `fusion_reactor` | power 10 | | 2.5 | 4 units |
| `capacitor` | power 2 | | 2 | |
| `thruster` | propulsion 3 | | 3 | |
| `torch_drive` | propulsion 9 | | 2.25 | 4 units |
| `gyro` | propulsion 3 | | 3 | none in slice wrecks |
| `rcs_pod` | propulsion 2 | | 2 | none in slice wrecks |
| `shield_emitter` | shields 3 | | 3 | no shields node in the slice (open question 8) |
| `radiator` | thermal 2 | | 2 | no thermal node (open question 8) |
| `cargo_bay`, `cargo_hold` | utility 1, 5 | | 1, 1.25 | no utility node (open question 8) |
| hull in `steel` / `automaton_plate` | hull 1 per footprint cell | | 1 | slopes count their footprint cell |
| hull in `composite` | hull 2 per footprint cell | | 2 | plan: 1.5; whole numbers keep the save's data integer |
| hull in `reactive` | – | | – | locked, in no slice wreck |

Example lines (field names as the plan's sketch):

```ron
(id: "automaton_cannon", ..., salvage: (family: "kinetic", data: 3)),
(id: "automaton_core", ..., salvage: (family: "logic", data: 4, materials: {"programmable_matter": 1})),
(id: "composite", ..., salvage: (family: "hull", data: 2)),   // per cell of hull in this material
```

### 4.4 Locks and the one new material

`locked: true` moves to (GDR-0063): `composite` and `reactive` (materials), `fusion_reactor`, `beam_laser`,
`torch_drive` (parts), `cruiser` and `capital` (frames); the automaton core, cannon and plate stay locked. Steel, the
armour block and the 1×1 slope stay open (validation needs a paint block, a bevel slope and one armour material).
Reactive armour, automaton plate and the capital frame have no node in the slice.

```ron
// materials/precursor.ron. PLACEHOLDER.
(
    id: "programmable_matter",
    name: "Programmable matter",
    summary: "Precursor matter that can host a copy of the ship-mind.",
    description: "Every [[part:logic_core|logic core]] holds some. Rogue automaton cores are the first source.",
    icon: "programmable-matter",   // needs icons/programmable-matter.svg
)
```

---

## 5. Texts

Content texts live in data (ADR-0018). UI chrome (banners, counters) stays in the client, keyed later; its wording
is listed in 5.3 so 7G and 7H use the same lines.

### 5.1 The start (`starts.ron`)

```ron
// starts.ron: how a new game begins. PLACEHOLDER NUMBERS (track 7J, 13 Oct 2026).
(
    id: "rim_awakening",
    name: "Awakening at the rim",
    summary: "You wake in a dead shipyard with one small ship and a drydock that still works.",
    description: "You wake in a dead shipyard at the rim of the galaxy. Its makers are gone; its drydock still answers. One small ship waits in the berth: take it out, strip the wrecks, and bring home what you learn.",
    ship: "rim_skiff",
    mind_capacity: 2,
    voyages: ["yard_approach"],
    map: (0.14, 0.78),            // proposed: the shipyard's place on the voyage map
)
```

The intro card shows the description as two lines, split after "answers.":

> You wake in a dead shipyard at the rim of the galaxy. Its makers are gone; its drydock still answers.
> One small ship waits in the berth: take it out, strip the wrecks, and bring home what you learn.

### 5.2 Voyage and encounter texts

In the RON above: each voyage's `summary` (the map panel) and `cleared` (the debrief line the first time its goal
falls), each encounter's `summary` (the site marker's tooltip and the map's site list) and `lore`.

The end of the slice is the Warden's gate's `cleared` line, followed by the banner:

> The Warden goes dark. The gate behind it stays cold: whatever opened it once is gone.
> **THE GATE TO THE OUTER REACH STAYS DARK · TO BE CONTINUED**

### 5.3 Debrief, milestone and HUD lines (client UI strings, from the plan)

| Moment | Line |
|---|---|
| Voyage map panel | `RIM · AUTOMATONS` chips; `RAIDERS · WRECKS`; `≈ 8 MIN · RECALL WHEN CLEAR`; `VOYAGE-READY` / `NOT VOYAGE-READY · USES BEAM LASER` / `3 CORES, YOUR MIND RUNS 2`; warning `NO CARGO · YOU CAN'T BRING SALVAGE HOME` |
| Arrival | `HALVEX DRIFT · RIM` (title), `SITES 0 / 5` |
| A site wakes / falls | `RAIDER PACK WAKES` / `SITE CLEARED` / `GOAL CLEARED · CORRAN REACH OPENS` |
| Salvage | `SALVAGE 5 / 8 · CARGO HOLD`, `HOLD FULL` |
| Recall | `RECALL` (enabled) / `RECALL · ENEMIES NEAR` (blocked) / `WARPING HOME · 3` |
| Loss | `SHIP LOST · RECALLED HOME` and `THE HAUL STAYS WITH THE WRECK` (last vessel); `FOCUS · <ESCORT NAME>` (an escort lives on) |
| Debrief on Home | `HAUL UNLOADED · 14 PARTS → KINETIC +18 · PROPULSION +9 · LOGIC +4`, `PROGRAMMABLE MATTER +3`, `RESEARCH · 1 READY` |
| Research | `AUTOMATON CANNON · NOW IN THE DRYDOCK`, `MIND CAPACITY 3`, `CRUISER FRAME · NOW IN THE DRYDOCK` |
| Drydock | `NEW` tag; `LOCKED · <NODE NAME>` rows |
| The end | `THE GATE TO THE OUTER REACH STAYS DARK · TO BE CONTINUED` (content, 5.2) |

Light milestone lines (GDR-0002's "light story"), shown once on Home; they could live as `description`s of the
start, the nodes and the voyages, or as a later `story/` kind (`galaxy-voyages.md` 4.6):

| When | Line |
|---|---|
| First haul home | "The lab wakes for the first time in a long while and starts taking the haul apart." |
| First research | "The drydock remembers a part it had forgotten." |
| Mind expansion I | "Your mind stretches over one more core. It answers like your own hand." |
| First ship lost | "The wreck stays out there. The ship does not: it re-forms in the berth, bonded to you." |
| The Warden beaten | the Warden's gate `cleared` line and the end banner (5.2) |

### 5.4 Lore lines for derelicts

On the encounters (above): the derelict field, the precursor wrecks, the core graveyard, the Warden. More fragments
for later (a `lore/` pool once scanning exists, or wreck `lore` fields):

- "Berth four is still reserved. The ship it was reserved for never docked."
- "A repair order, signed by no one: keep the drydock running; someone will need it."
- "The raiders were built to keep the lanes clean. They never stopped. They only forgot what clean meant."
- "Every core here once held a copy of something. Most hold nothing now. One of them held you."

---

## 6. The `escort` behavior

```ron
// The ship-mind's escort template: fight what the leader has locked, and back off when badly hurt. Two rules, so it
// fits a tamed automaton core (two rule slots) as well as a logic core. Written because the `skirmisher` orbits too
// much to bring fixed guns to bear (docs/sim.md "Sandbox fights at the new speeds": 3–5 of 12 waves won), while a
// pilot who simply closes in and keeps the nose on the target won 11–12 of 12. PLACEHOLDER numbers.
(
    id: "escort",
    name: "Escort",
    summary: "Fight whatever your leader has locked, nose first, and back off when badly hurt.",
    description: "Attacks only the [[behavior_node:locked|leader's lock]], so it never wakes a sleeping pack on its own; without a lock it holds where it is. Aims at the target's core, which leaves more of the wreck to salvage.",
    tags: ["escort", "attack"],
    target: "locked(core)",
    rules: [
        "when hull < 30% and target_on_me -> kite(150%) # badly hurt and under its guns: open the gap",
        "always -> approach(70%) # nose on the leader's target, inside every gun's reach",
    ],
)
```

Rule by rule:

- **`target locked(core)`**: the target is whatever the leader (the flagship, `ShipCommand::Leader`) has locked, or
  its behavior target when the leader runs a behavior. Escorts never pick their own fights: `nearest` would send
  them at any active enemy vessel in the world, including a site the player meant to sneak past. The aim point is
  the target's nearest working core: a raider or hunter dies with its only core, sooner and with more intact parts
  left for salvage; on the Warden it aims at the nearest section's core.
- **`when hull < 30% and target_on_me -> kite(150%)`**: a badly damaged escort that the target is shooting at keeps
  one and a half times its own reach away (90 m with pulse lasers: outside a raider's 75 m cannon). It stays in the
  fight's area and keeps whatever it has in its hold (there are no repairs on a voyage, and its cargo stays in its
  wreck if it dies). Ships with RCS pods back off nose-on; ships with engines only at the back turn away and come
  back (`kite`'s rule). While the target shoots at someone else, it keeps attacking.
- **`always -> approach(70%)`**: close in to 70 % of the reach of its longest clear gun, then face the target and keep
  pace. Guns fire at will within 0.933 × their own reach, so 70 % of the longest reach keeps every gun firing whose
  reach is at least three quarters of the longest (pulse lasers with cannons; beams with pulse lasers, just). No orbiting:
  fixed guns stay on the target, which is what the winning "closing" stand-in did. Without a lock this rule fails
  (no target), no rule matches, and the escort brakes and holds.

What it cannot do with today's vocabulary: **keep near the leader**. Every action flies relative to the target, and
no condition or action knows the leader's position, so between fights (no lock) an escort stays where the last
fight ended until the player locks the next target, then flies there on its own (about 25 s for 500 m at 20 m/s).
The galaxy plan already proposes the node that fixes it (`galaxy-voyages.md` 5.2: action `escort(distance)`, "keep
station on the leader when no target"). With it, the template becomes three rules, the station-keeping one last so
it runs only when there is no lock:

```text
target locked(core)
when hull < 30% and target_on_me -> kite(150%)
always -> approach(70%)
always -> escort(30m)      # proposed node: keep station 30 m off the leader while it has no lock
```

(A 2-slot drone would then run only the first two; for drones the hurt rule is the one to drop.)

Use: escort designs name `escort` as their behavior (the BEHAVIOR screen's templates list it). The flagship should
not: when the player switches focus to an escort, the flagship becomes the unpiloted one and has no leader, so
`locked` gives it nothing to do.

---

## 7. Open questions for the lead

1. **The starter's strength.** 2.3 /s and 24 HP against 2 + 2 + 3 raiders with no repairs, and the raiders outrange
   its lasers (75 m vs 60 m; intended as the cannon's hook). Recommend: measure with the runner first; tune
   encounter counts before the ship; the armed variant (shoulder lasers, +1 reactor) is the second lever.
2. **RCS pods or gyros on the starter.** Pods (chosen): 0.8 s per 90°, strafing and reversing, cheap in power, but
   18.6 m/s² below 11 m/s as boosters (GDR-0058's open 200 vs 150 kN). A gyro pair instead turns it in about 0.7 s
   but needs 26 MW against 24 (a power deficit). Recommend pods.
3. **The hunter is a brute.** One automaton core (6 MW, control 4) powers exactly 2 cannons + 1 thruster, so it is
   heavy and slow (7.5 m/s², 1.03 s per 90°). Alternatives: a second thruster (8 MW, the guns brown out at full
   thrust) or another name. Recommend keep it as is.
4. **The Warden.** 9×7 instead of 9×9 (two more heart rows add about 13 t). After a split the heart is an
   immobile, unarmed vessel with two bubbles (12 capacity, 3 /s regen, energy fully absorbed): pulse lasers alone
   (2.3 /s) can never break it, so the slice's end needs cannons or beams, which research gives first. Is that the
   intended gate? A two-rule `warden` behavior (`target threat`, face when in range) would read more like a boss
   than `raider_basic`. Recommend raider_basic for the slice.
5. **Encounter variants by count**: separate entries (`raider_pair`, `raider_pack`, `raider_den`; this draft) or a
   per-site count override in the voyage. Recommend separate entries (no schema).
6. **Wrecks with intact cores.** With a damage share of 0.3 about one core in three dies in a graveyard; a
   `spare: [Tag("core")]` field on wreck entries would guarantee them. Should spawn damage leave fragments?
   Recommend no field and no fragments from spawn damage.
7. **Programmable matter after GDR-0063.** Fragments make every kill a source; the plan's node costs (1 + 2 + 4)
   would be paid on the first voyage. This draft uses 2 + 5 + 10; also needed: a fragment's place in the strip order
   (suggest: 1 PM counts as 4 data) and its cargo (1 unit).
8. **Families without nodes** (shields, thermal, utility). Collect their parts and keep the data for later trees
   (this draft), or give those parts no `salvage` block in the slice so holds fill with useful parts. Recommend
   collect, and dim empty families on the research screen.
9. **Proposed fields**: voyage `minutes` (the map's "≈ 8 MIN") and `cleared` (first-clear line), start `map` (the
   shipyard on the voyage map), encounter `lore`/`description` (common header, already allowed). For 7A/7E/7I.
10. **Escorts keeping station** need one new action node (`escort(distance)` from the galaxy plan) in 7H or 7J;
    without it escorts hold between fights. Also: `locked` returns any locked ship, a wreck included, so an escort
    would shoot a wreck the player locks; check in 7H whether the player can lock wrecks.
11. **Map order vs the galaxy mockup**: in `phone-galaxy-map.png` Corran Reach lies between the shipyard and Halvex
    Drift; the plan's chain visits Halvex first. Recommend keep the plan's order (its ids are in the plan, ADR and
    save examples); swap the names only if the map should match the mockup.
12. **A starter escort design** (a 1-core `rim_picket` among the BLUEPRINTS templates) would make the first mind
    expansion pay off at once. Recommend adding it in 7J once Mind expansion I is tuned.
13. **Derelict ship ids**: the wrecks reuse `sample_early` and `sample_mid` (tagged `sample`). Fine for the slice;
    dedicated derelict blueprints can come with the galaxy's wreck fields.
