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
| [0004](0004-grid-ship-building-no-wiring.md) | Ships are built on a grid of parts with no wiring: power pool per piece, heat and radiators, adjacency bonuses | Accepted; engine rule superseded by [GDR-0039](0039-engine-exhaust-hurts-parts.md) | concept phase |
| [0005](0005-fully-physical-ships.md) | Ships are fully physical and break apart | Accepted | concept phase |
| [0006](0006-ship-scale-no-fixed-cap.md) | Ships are ~60×60 blocks and up, with no fixed block cap | Accepted | 2026-10-10 |
| [0007](0007-ship-style-2a.md) | Ships use style 2a: flat tiles with seams, glowing icons, a soft teal hull glow | Accepted; machine-tile rule superseded by [GDR-0069](0069-machines-drawn-as-components.md) | 2026-10-10 |
| [0008](0008-ui-ship-mind-interface.md) | The UI is a diegetic ship-mind interface with a floating HUD, landscape only | Accepted | 2026-10-10 |
| [0009](0009-stick-and-lock-controls.md) | Controls: stick + lock with an always-on flight computer; letting go brakes | Accepted; PC/WASD part superseded by [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md) | 2026-10-10 |
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
| [0024](0024-tech-trees-no-mk-levels.md) | Big tech trees per family, with node kinds instead of Mk levels | Accepted | concept phase |
| [0025](0025-research-from-salvage.md) | Research comes from salvage and scans, and only counts once brought home | Accepted | 2026-10-10 |
| [0026](0026-procedural-galaxy-and-bands.md) | One procedural galaxy, five bands from rim to core, gated by guardians | Accepted | 2026-10-10 |
| [0027](0027-races.md) | Rogue automatons and four alien races, one per band, mainly for variety and tech | Accepted | 2026-10-10 |
| [0028](0028-clusters-and-voyages.md) | Clusters are large; a default voyage covers one cluster in 15–30 minutes | Accepted | 2026-10-10 |
| [0029](0029-mild-death-bonded-matter.md) | Death is mild: bonded matter re-forms your ship at home; the unbonded haul stays as a wreck | Accepted | 2026-10-10 |
| [0030](0030-no-economy.md) | No economy: no credits, no trading; parts keep build-point costs for balance | Accepted | concept phase |
| [0031](0031-materials-and-fabrication.md) | No generic scrap: materials come from places, and the fabricator resolves recipe chains without factory logistics | Accepted | 2026-10-10 |
| [0032](0032-balancing-arena-evolution.md) | Balance with an AI-vs-AI arena and evolutionary search; enemies are handcrafted plus evolved | Accepted | concept phase |
| [0033](0033-multiplayer-coop-and-arena.md) | Multiplayer later: co-op voyages in the host's galaxy and arena PvP with your own ships | Accepted | 2026-10-10 |
| [0034](0034-behavior-first-match-fire-at-will.md) | Behaviors pick the first matching rule about ten times a second; each gun fires at will along its facing; no match means hold | Accepted | 2026-10-11; 2026-10-12 |
| [0035](0035-rule-slots-per-core.md) | Rule slots: six per logic core, two per automaton core; the lowest rules switch off first | Accepted | 2026-10-11; 2026-10-12 |
| [0036](0036-behavior-text-format.md) | The behavior text format: one line per rule, `when ... and ... -> action`, no `end`, no `or` | Accepted | 2026-10-11; 2026-10-12 |
| [0037](0037-behavior-vocabulary-as-content.md) | The behavior vocabulary is content; v1 has 24 nodes, one target line with aim priorities, and the raider pack as data | Accepted | 2026-10-11; 2026-10-12 |
| [0038](0038-logic-cores-control-parts.md) | Each logic core runs up to N parts; mind capacity caps the running cores and so the ship's size | Accepted | 2026-10-12 |
| [0039](0039-engine-exhaust-hurts-parts.md) | Engines may sit anywhere; their exhaust damages and heats the parts in its path | Accepted | 2026-10-12 |
| [0040](0040-keyboard-flight-and-smart-lock-aim.md) | PC keyboard flight controls: rotate + thrust + strafe, smart lock aim with A/D nudge | Accepted | 2026-10-12 |
| [0041](0041-engine-rated-speed.md) | Top speed comes from each engine's rated speed; no drag; a calm cruise flame at the cap | Accepted; fade refined by [GDR-0055](0055-slower-crisper-turning.md) | 2026-10-12 |
| [0042](0042-agility-target.md) | Agility target "Agile": the Halvex Runner turns 90° in ~0.8 s and reaches ~22 m/s in ~2.5 s; small ships are snappier | Accepted; turning superseded by [GDR-0055](0055-slower-crisper-turning.md) | 2026-10-12 |
| [0043](0043-shields-as-energy-bubbles.md) | Shields are drawn as glowing energy bubbles | Accepted | 2026-10-12 |
| [0044](0044-control-cost-priority-idle-dormant-newest.md) | Parts have a control cost; parts over the cores' limit idle by power priority; the newest vessels go dormant first | Accepted | 2026-10-12 |
| [0045](0045-shields-absorb-rams-no-friendly-ram-damage.md) | Shields absorb ram damage; vessels of the same team bump without ram damage | Accepted | 2026-10-12 |
| [0046](0046-exhaust-hits-own-parts-and-enemies.md) | Engine exhaust hits the ship's own parts and enemies, not other friendly vessels | Accepted | 2026-10-12 |
| [0047](0047-ramming-crushes-inward-bumps-free.md) | Ramming crushes inward; bumps below 100 kN·s are free | Accepted | 2026-10-12 |
| [0048](0048-no-friendly-fire.md) | No friendly fire: shots pass through the shooter's own vessels | Accepted | 2026-10-12 |
| [0049](0049-engine-plume-v1.md) | Engine plume v1: damage and heat scale with thrust, blocked plumes keep thrust, enemy bubbles absorb plumes | Accepted (provisional, lead) | 2026-10-12 |
| [0050](0050-fleet-bar.md) | The fleet bar: a strip at the top centre, shown with two or more vessels or a dormant one; a tap focuses, a tap on a dormant vessel wakes it | Accepted (provisional, lead) | 2026-10-12 |
| [0051](0051-drydock-v1.md) | Drydock v1 comes next: unlimited parts, stats as information, test-fly into the sandbox, the mockup's look | Accepted | 2026-10-12 |
| [0052](0052-drydock-stats-full-use.md) | Drydock stats show the ship at full use and full power; up to ~120 % heat reads "runs hot" | Accepted | 2026-10-12 |
| [0053](0053-drydock-v1-lead-defaults.md) | Drydock v1 defaults: frames, tools, mirror, keys and test flight | Accepted (provisional, lead); palette rule superseded by [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md) | 2026-10-13 |
| [0054](0054-drydock-locked-parts-tools-saving-sharing.md) | Drydock: enemy parts locked until unlocked; fill and bevel confirmed; the working design autosaves; designs are stored and can be shared | Accepted; hiding locked parts superseded for the career by [GDR-0065](0065-phase-7-screens-and-escort-action.md) | 2026-10-13 |
| [0055](0055-slower-crisper-turning.md) | Turning is slower and crisper; RCS pods are the strong turners; top speed is capped by the whole speed | Accepted | 2026-10-13 |
| [0056](0056-drydock-wave-2-defaults.md) | Drydock wave 2 defaults: editing gestures, stats wording, saving behaviour | Accepted (provisional, lead) | 2026-10-13 |
| [0057](0057-behavior-editor-v1.md) | Behavior editor v1: its own screen from the drydock; behaviors are saved on their own and designs pick one; test by watching or flying alongside; graph and text views | Accepted | 2026-10-13 |
| [0058](0058-turning-v2-defaults.md) | Turning v2 defaults: the numbers, RCS couples and when engines help turning | Accepted (provisional, lead) | 2026-10-13 |
| [0059](0059-behavior-editor-wave-a-defaults.md) | Behavior editor wave A defaults: editing rules, storage and tests, the screen | Accepted (provisional, lead) | 2026-10-13 |
| [0060](0060-behavior-editor-wave-b-defaults.md) | Behavior editor wave B defaults: editing gestures, saving and linking, the live panel | Accepted (provisional, lead) | 2026-10-13 |
| [0061](0061-fight-balance-after-turning-v2.md) | Fight balance after turning v2: raiders orbit inside the beams, the skirmisher keeps its nose on the target, shields can collapse | Accepted | 2026-10-13 |
| [0062](0062-first-playable-loop.md) | The first playable loop: start at home, short hand-made rim voyages, mind capacity 2 → 4 | Accepted | 2026-10-13 |
| [0064](0064-lock-feed-forward-and-ai-flight-fixes.md) | The lock and `face` lead a moving target's turn; AI flight fixes from the balance study | Accepted | 2026-10-13 |
| [0063](0063-salvage-and-research-v1.md) | Salvage and research v1: fly over wrecks, the haul is dismantled at home into research data, unlocks are unlimited | Accepted | 2026-10-13 |
| [0065](0065-phase-7-screens-and-escort-action.md) | Phase 7 screens: debrief panel, research tree, star map, recall under the cargo, fleet formation, locked parts shown dimmed; an `escort` action | Accepted | 2026-10-13 |
| [0066](0066-phase-7-wave-1-defaults.md) | Phase 7 wave 1 defaults: salvage values and fragments, cargo and collecting, the career model, the home screen | Accepted (provisional, lead) | 2026-10-13 |
| [0067](0067-rams-pass-friendly-bubbles.md) | Rams pass friendly shield bubbles, like shots and plumes | Accepted | 2026-10-13 |
| [0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md) | A voyage site clears when its enemies are destroyed or disarmed; only armed enemies block recall | Accepted | 2026-10-13 |
| [0069](0069-machines-drawn-as-components.md) | Machines are drawn as their own components; engine nozzles reach past the hull | Accepted | 2026-10-13 |
| [0070](0070-phase-7-wave-2-defaults.md) | Phase 7 wave 2 defaults: flight and AI numbers, voyage clearing details, the career save, the drydock gating | Accepted (provisional, lead) | 2026-10-13 |
| [0071](0071-shipyard-home-long-u.md) | The home screen is the shipyard: a long U-shaped dock, the building area fills the U, the gate at the open side | Accepted | 2026-10-13 |
| [0072](0072-shipyard-parts-voyage-flight-defaults.md) | Defaults of the shipyard home, the machine drawings and voyage flight | Accepted (provisional, lead) | 2026-10-13 |
| [0073](0073-slice-tuning-v1.md) | Slice tuning v1: smaller packs, a gentler first voyage, research costs about halved | Accepted | 2026-10-13 |
| [0074](0074-test-range-waves-emergency-recall-cannon.md) | Test-range waves follow the flown ship; recall also works when the fleet is disarmed; the cannon becomes an upgrade | Accepted | 2026-10-13 |
<!-- index:end -->

## How to add, supersede and deprecate

- **Add:** copy [`0000-template.md`](0000-template.md) to the next free number (`NNNN-short-slug.md`), keep it readable in under two minutes, add its row above and link it from `docs/concept-notes.md`. Agents write `Proposed`; Niklas's decisions are `Accepted`; lead-session defaults are `Accepted (provisional, lead)`.
- **Supersede:** never rewrite a decision. Write a new record with a `Supersedes: GDR-NNNN` line, then change only the old record's status to `Superseded by GDR-MMMM` (and its row here).
- **Deprecate:** when a decision stops applying and nothing replaces it, set its status to `Deprecated` and add one dated line saying why.

## Open

Not decided yet, so no record. Write one when Niklas decides.

- **Fight length** at the new speeds: `raider_basic` stand-off and orbit (85% / 67% of its reach) are on a knife edge around the beam range; the `skirmisher` wingmen win only 3–5 of 12 waves (`docs/sim.md` design question 4 and "Sandbox fights at the new speeds", measured before the slower turning of GDR-0055). Speed, acceleration and turning are decided ([GDR-0041](0041-engine-rated-speed.md), [GDR-0042](0042-agility-target.md), [GDR-0055](0055-slower-crisper-turning.md)).
- **Feel of "fly that way"** for forward-only ships: let the nose swing past the stick ("drift turn", current behaviour, recommended) or keep the nose on the stick (`docs/sim.md` design question 3).
- **Mind capacity numbers** beyond the slice (the slice: start 2, research to 4, [GDR-0062](0062-first-playable-loop.md); the exclusive resource) and N parts per core type ([GDR-0013](0013-vessels-and-piloting.md), [GDR-0038](0038-logic-cores-control-parts.md), [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md)); lead placeholders until then.
- **Engine plume** numbers and the v1 defaults ([GDR-0049](0049-engine-plume-v1.md): damage and heat scale with output, blocked plumes keep thrust, enemy bubbles absorb plumes) (`docs/sim.md` design question 16). No push and the faint reach are decided ([GDR-0055](0055-slower-crisper-turning.md)).
- **Other research sources** besides salvage and scans ([GDR-0025](0025-research-from-salvage.md)).
- **Tree size per family** that stays fun instead of tedious ([GDR-0024](0024-tech-trees-no-mk-levels.md)).
- **Overdrive** details ([GDR-0004](0004-grid-ship-building-no-wiring.md)); **missiles, disruption and drones** are not built yet ([GDR-0018](0018-weapon-families.md)).
- **New game+** (what carries over) and the **working title**.

Parked (maybe later): location-based play (light version: your real location seeds your home region), co-op
raids (after base co-op works), shared blueprints + behavior trees as AI opponents (bounty hunters, async PvP).
