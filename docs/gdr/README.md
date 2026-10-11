# Game Design Records (GDR)

Gameplay and design decisions for the space game: ship rules, combat, controls, progression, world, look.
Technical decisions are in [`docs/adr`](../adr/README.md). [`docs/concept-notes.md`](../concept-notes.md) is the
overview and links to both series. Format and status rules: `CLAUDE.md`, section "Decision records".

## Index

<!-- index:start -->
| GDR | Title | Status | Date |
|---|---|---|---|
| [0001](0001-record-game-design-decisions.md) | Record game design decisions | Accepted | 2026-10-11 |
<!-- index:end -->

## How to add, supersede and deprecate

- **Add:** copy [`0000-template.md`](0000-template.md) to the next free number (`NNNN-short-slug.md`), keep it readable in under two minutes, add its row above and link it from `docs/concept-notes.md`. Agents write `Proposed`; Niklas's decisions are `Accepted`; lead-session defaults are `Accepted (provisional, lead)`.
- **Supersede:** never rewrite a decision. Write a new record with a `Supersedes: GDR-NNNN` line, then change only the old record's status to `Superseded by GDR-MMMM` (and its row here).
- **Deprecate:** when a decision stops applying and nothing replaces it, set its status to `Deprecated` and add one dated line saying why.

## Open

Not decided yet, so no record. Write one when Niklas decides.

- **Ship speed and fight length.** With concept-board numbers ships reach ~0.7 m/s² and ~10 m/s, and a Halvex Runner kills three raiders ~15 s after contact. Niklas tunes this with live sliders in the browser build (lead's recommendation: raise thrust in content, `docs/sim.md` design question 1). The raider brain's distances (question 4) are tuned with it.
- **Feel of "fly that way"** for forward-only ships: let the nose swing past the stick ("drift turn", current behaviour, recommended) or keep the nose on the stick (`docs/sim.md` design question 3).
- **Engine placement:** anywhere with open space behind, or only on the rear edge? The evolution lab found nose and mid-ship engines legal.
- **Mind capacity numbers** (start value, growth, the exclusive resource), and whether it still limits ship size ([GDR-0006](0006-ship-scale-no-fixed-cap.md) vs [GDR-0013](0013-vessels-and-piloting.md)).
- **Other research sources** besides salvage and scans ([GDR-0025](0025-research-from-salvage.md)).
- **Tree size per family** that stays fun instead of tedious ([GDR-0024](0024-tech-trees-no-mk-levels.md)).
- **Overdrive** details ([GDR-0004](0004-grid-ship-building-no-wiring.md)); **missiles, disruption and drones** are not built yet ([GDR-0018](0018-weapon-families.md)).
- **New game+** (what carries over) and the **working title**.

Parked (maybe later): location-based play (light version: your real location seeds your home region), co-op
raids (after base co-op works), shared blueprints + behavior trees as AI opponents (bounty hunters, async PvP).
