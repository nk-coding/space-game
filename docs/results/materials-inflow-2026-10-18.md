# Material inflow per voyage, 18 Oct 2026 (GDR-0108)

What a voyage's haul gives in materials at home before and after the materials economy of
[GDR-0108](../gdr/0108-materials-economy-workshop.md): most stripped parts break (`SalvageTuning::intact_chance`
0.15), broken parts and hull give the workshop's `broken` share (0.25) of their recipe (hull a unit of its armour
per cell) on arrival, intact parts give `dismantle` (0.4) when dismantled.

**How**: the balance harness's voyage mode (`../space-game-balance`, `src/voyage.rs`, commit "Voyage mode measures
the material inflow"), the game's headless voyage runner over seeds 1–200 of the four slice voyages with the
starter skiff and the Runner with the skiff as escort, the content of this commit read at run time:

```sh
target/release/space-game-balance --content ../space-game/content/base --voyages yard_approach,halvex_drift,corran_reach,wardens_gate \
  --fleets rim_skiff,halvex_runner+rim_skiff --seeds 1-200 --threads 4
```

Per voyage that came home (lost voyages bring nothing): **items** in the holds, **intact** non-hull parts among
them, **whole** (before: every part as if intact and dismantled for its whole recipe, hull a unit per cell, plus the
fragments: GDR-0031 as written, the haul as generous as without the intact roll), **now** (broken parts and hull at
25 % on arrival plus the intact parts dismantled at 40 %, keeping none, plus the fragments), material units summed
over materials; **fleet cost** is what the fleet's parts take to fabricate, for scale. The game before this change
gave programmable matter only (1 per salvaged core and fragment). Sim at intact chance 0.15; the "whole" column is
computed from the same hauls (the roll changes only which items are broken, not what is collected). Load average
during the run about 2.

# Voyages: 200 seeds (1..200), 1600 runs in 22.5 s wall clock; clear / home / lost / out of time

| voyage | fleet | variant | clear | home | lost | out of time | clear % | items | intact | whole | now | fleet cost |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| yard_approach | rim_skiff | base | 113 | 67 | 20 | 0 | 56 | 3.1 | 0.0 | 4.8 | 2.2 | 108 |
| yard_approach | halvex_runner+rim_skiff | base | 147 | 47 | 6 | 0 | 74 | 8.2 | 0.2 | 18.3 | 6.6 | 289 |
| halvex_drift | rim_skiff | base | 71 | 91 | 38 | 0 | 36 | 2.2 | 0.1 | 4.6 | 1.9 | 108 |
| halvex_drift | halvex_runner+rim_skiff | base | 135 | 48 | 17 | 0 | 68 | 7.3 | 0.4 | 24.5 | 8.1 | 289 |
| corran_reach | rim_skiff | base | 37 | 131 | 32 | 0 | 18 | 1.8 | 0.0 | 2.9 | 1.6 | 108 |
| corran_reach | halvex_runner+rim_skiff | base | 85 | 104 | 10 | 1 | 42 | 6.1 | 0.1 | 12.9 | 5.5 | 289 |
| wardens_gate | rim_skiff | base | 30 | 139 | 31 | 0 | 15 | 2.3 | 0.1 | 6.5 | 2.7 | 108 |
| wardens_gate | halvex_runner+rim_skiff | base | 86 | 106 | 8 | 0 | 43 | 6.8 | 0.4 | 21.9 | 7.9 | 289 |

| voyage | fleet | variant | materials now per voyage home (mean) |
|---|---|---|---|
| yard_approach | rim_skiff | base | automaton_plate 0.49, ceramic 0.04, copper 0.11, crystal 0.01, programmable_matter 1.47, steel 0.11 |
| yard_approach | halvex_runner+rim_skiff | base | automaton_plate 1.47, ceramic 0.49, copper 0.67, crystal 0.26, programmable_matter 3.01, steel 0.73 |
| halvex_drift | rim_skiff | base | automaton_plate 0.23, ceramic 0.14, copper 0.28, crystal 0.15, programmable_matter 0.94, steel 0.17 |
| halvex_drift | halvex_runner+rim_skiff | base | automaton_plate 0.93, ceramic 0.96, composite 0.09, copper 1.82, crystal 1.01, programmable_matter 2.19, steel 1.11 |
| corran_reach | rim_skiff | base | automaton_plate 0.14, ceramic 0.05, copper 0.06, crystal 0.02, programmable_matter 1.17, steel 0.13 |
| corran_reach | halvex_runner+rim_skiff | base | automaton_plate 0.93, ceramic 0.27, composite 0.02, copper 0.65, crystal 0.28, programmable_matter 2.80, steel 0.60 |
| wardens_gate | rim_skiff | base | automaton_plate 0.56, ceramic 0.08, copper 0.11, crystal 0.02, programmable_matter 1.83, steel 0.14 |
| wardens_gate | halvex_runner+rim_skiff | base | automaton_plate 2.12, ceramic 0.55, copper 0.66, crystal 0.27, programmable_matter 3.28, steel 0.99 |

## Reading

- The inflow is about a third of the whole value (skiff 4.8 → 2.2 units on the yard approach, the Runner's fleet
  18.3 → 6.6; 25–45 % across the voyages). Most of it is programmable matter from the cores' fragments (unchanged,
  the research currency); the fabrication materials (copper, ceramic, crystal, steel) come to about 0.3 a voyage for
  the skiff's small hold and 2–5 for the Runner's fleet, against 6–10 a part (a pulse laser 6, a thruster 6, a
  fission reactor 8). A new part takes a few voyages with a good hold, a second skiff (108 units) dozens: collecting
  matters again.
- Intact parts are rare: 0–0.4 a voyage, so keeping one is an event.
- The rim's automatons are made of automaton plate, steel and a little copper (their cannons and cores), so the rim
  gives plate and PM most; crystal and ceramic come from derelicts and thrusters. Open for play: the skiff's hold may
  make the start too slow (the start's materials cover a spare laser and a thruster), and the automaton plate has no
  use but the reverse-engineered cannon until the automaton forge.
