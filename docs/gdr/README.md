# Game Design Records (GDR)

Gameplay and design decisions for the space game: ship rules, combat, controls, progression, world, look.
Technical decisions are in [`docs/adr`](../adr/README.md). [`docs/concept-notes.md`](../concept-notes.md) is the
overview and links to both series. Format and status rules: `CLAUDE.md`, section "Decision records".

## Index

<!-- index:start -->
| GDR | Title | Status | Date |
|---|---|---|---|
| [0001](0001-record-game-design-decisions.md) | Record game design decisions | Accepted | 2026-10-11 |
| [0002](0002-awakened-ship-mind-light-story.md) | The player is an awakened ship-mind at a precursor shipyard; the story stays light | Accepted | 2026-10-10 |
| [0003](0003-pilot-one-ship-with-escort.md) | You pilot one ship with a small scripted escort; the fleet is secondary | Superseded by [GDR-0013](0013-vessels-and-piloting.md) | 2026-10-10 |
| [0004](0004-grid-ship-building-no-wiring.md) | Ships are built on a grid of parts with no wiring: power pool per piece, heat and radiators, adjacency bonuses | Accepted | concept phase |
| [0005](0005-fully-physical-ships.md) | Ships are fully physical and break apart | Accepted | concept phase |
| [0006](0006-ship-scale-no-fixed-cap.md) | Ships are ~60×60 blocks and up, with no fixed block cap | Accepted | 2026-10-10 |
| [0007](0007-ship-style-2a.md) | Ships use style 2a: flat tiles with seams, glowing icons, a soft teal hull glow | Accepted | 2026-10-10 |
| [0008](0008-ui-ship-mind-interface.md) | The UI is a diegetic ship-mind interface with a floating HUD, landscape only | Accepted | 2026-10-10 |
| [0009](0009-stick-and-lock-controls.md) | Controls: stick + lock with an always-on flight computer; letting go brakes | Accepted | 2026-10-10 |
| [0010](0010-turning-sources.md) | Ships turn with core assist, gyros, RCS pods and their engines (gimbal and differential thrust) | Accepted | 2026-10-11 |
| [0011](0011-locked-flight-rear-engines-trade-off.md) | Ships with rear-only engines can't brake or strafe while locked: a design trade-off | Accepted | 2026-10-11 |
| [0012](0012-halvex-runner-follows-rules.md) | The Halvex Runner's layout follows the drydock rules; the content blueprint is the source of truth | Accepted | 2026-10-11 |
| [0013](0013-vessels-and-piloting.md) | Every piece with a working logic core is a vessel; you pilot one and switch focus instantly | Accepted | 2026-10-11 |
| [0014](0014-power-tiers-and-weights.md) | Power is allocated by tiers and weights with partial output; logic cores need power | Accepted | 2026-10-11 |
| [0015](0015-layered-behavior.md) | Behavior is layered: rule lists over built-in maneuvers over an always-on flight computer | Accepted | concept phase |
| [0016](0016-combat-pace-and-aiming.md) | Combat is real-time, fast-leaning and mid-range; fixed guns are aimed by turning with a lock | Accepted | 2026-10-10 |
| [0017](0017-combat-support-repair-capture-sensors.md) | Repair drones mid-fight, capture by killing all power, moderate sensors, a minimal HUD | Accepted | 2026-10-10 |
| [0018](0018-weapon-families.md) | All six weapon families; the capacitor bonus is for lasers only; guns fire along their facing | Accepted | concept phase; 2026-10-11 |
| [0019](0019-damage-model.md) | Damage: outer blocks first, penetration by weapon type, ricochets off slopes, chain reactions, ramming | Accepted | 2026-10-10; 2026-10-11 |
| [0020](0020-damage-v1-defaults.md) | Damage v1 defaults: hitscan lasers, kinetic projectiles with ricochet, chain reactions later | Accepted (provisional, lead) | 2026-10-11 |
| [0021](0021-shields.md) | Shields are one bubble per powered emitter; bubbles stack and collapse when emptied | Accepted | 2026-10-10; 2026-10-11 |
| [0022](0022-heat-per-part.md) | Heat is simulated per part, with shutdown hysteresis | Accepted (provisional, lead) | 2026-10-11 |
| [0023](0023-shot-leading.md) | AI ships lead their shots; the player's lock aims at the lead point for projectile weapons | Accepted (provisional, lead) | 2026-10-11 |
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
