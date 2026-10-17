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
| [0020](0020-damage-v1-defaults.md) | Damage v1 defaults: hitscan lasers, kinetic projectiles with ricochet, chain reactions later | Accepted (provisional, lead); chain reactions built by [GDR-0084](0084-chain-reactions-v1.md) | 2026-10-11 |
| [0021](0021-shields.md) | Shields are one bubble per powered emitter; bubbles stack and collapse when emptied | Accepted; 2026-10-16 the point-blank pass is the muzzle's only | 2026-10-10; 2026-10-11 |
| [0022](0022-heat-per-part.md) | Heat is simulated per part, with shutdown hysteresis | Accepted (Niklas, 17 Oct 2026) | 2026-10-11 |
| [0023](0023-shot-leading.md) | AI ships lead their shots; the player's lock aims at the lead point for projectile weapons | Accepted (Niklas, 17 Oct 2026) | 2026-10-11 |
| [0024](0024-tech-trees-no-mk-levels.md) | Big tech trees per family, with node kinds instead of Mk levels | Accepted | concept phase |
| [0025](0025-research-from-salvage.md) | Research comes from salvage and scans, and only counts once brought home | Accepted | 2026-10-10 |
| [0026](0026-procedural-galaxy-and-bands.md) | One procedural galaxy, five bands from rim to core, gated by guardians | Accepted; lanes superseded by [GDR-0078](0078-galaxy-exploration-v1.md) | 2026-10-10 |
| [0027](0027-races.md) | Rogue automatons and four alien races, one per band, mainly for variety and tech | Accepted | 2026-10-10 |
| [0028](0028-clusters-and-voyages.md) | Clusters are large; a default voyage covers one cluster in 15–30 minutes | Accepted; lanes superseded by [GDR-0078](0078-galaxy-exploration-v1.md) | 2026-10-10 |
| [0029](0029-mild-death-bonded-matter.md) | Death is mild: bonded matter re-forms your ship at home; the unbonded haul stays as a wreck | Accepted | 2026-10-10 |
| [0030](0030-no-economy.md) | No economy: no credits, no trading; parts keep build-point costs for balance | Accepted | concept phase |
| [0031](0031-materials-and-fabrication.md) | No generic scrap: materials come from places, and the fabricator resolves recipe chains without factory logistics | Accepted | 2026-10-10 |
| [0032](0032-balancing-arena-evolution.md) | Balance with an AI-vs-AI arena and evolutionary search; enemies are handcrafted plus evolved | Accepted | concept phase |
| [0033](0033-multiplayer-coop-and-arena.md) | Multiplayer later: co-op voyages in the host's galaxy and arena PvP with your own ships | Accepted | 2026-10-10 |
| [0034](0034-behavior-first-match-fire-at-will.md) | Behaviors pick the first matching rule about ten times a second; each gun fires at will along its facing; no match means hold | Accepted; think stagger replaced by [ADR-0037](../adr/0037-simultaneous-fire-within-a-tick.md) | 2026-10-11; 2026-10-12 |
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
| [0050](0050-fleet-bar.md) | The fleet bar: a strip at the top centre, shown with two or more vessels or a dormant one; a tap focuses, a tap on a dormant vessel wakes it | Accepted (Niklas, 17 Oct 2026) | 2026-10-12 |
| [0051](0051-drydock-v1.md) | Drydock v1 comes next: unlimited parts, stats as information, test-fly into the sandbox, the mockup's look | Accepted | 2026-10-12 |
| [0052](0052-drydock-stats-full-use.md) | Drydock stats show the ship at full use and full power; up to ~120 % heat reads "runs hot" | Accepted | 2026-10-12 |
| [0053](0053-drydock-v1-lead-defaults.md) | Drydock v1 defaults: frames, tools, mirror, keys and test flight | Accepted (Niklas, 17 Oct 2026); palette rule superseded by [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0054](0054-drydock-locked-parts-tools-saving-sharing.md) | Drydock: enemy parts locked until unlocked; fill and bevel confirmed; the working design autosaves; designs are stored and can be shared | Accepted; hiding locked parts superseded for the career by [GDR-0065](0065-phase-7-screens-and-escort-action.md) | 2026-10-13 |
| [0055](0055-slower-crisper-turning.md) | Turning is slower and crisper; RCS pods are the strong turners; top speed is capped by the whole speed | Accepted | 2026-10-13 |
| [0056](0056-drydock-wave-2-defaults.md) | Drydock wave 2 defaults: editing gestures, stats wording, saving behaviour | Accepted (Niklas, 17 Oct 2026); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0057](0057-behavior-editor-v1.md) | Behavior editor v1: its own screen from the drydock; behaviors are saved on their own and designs pick one; test by watching or flying alongside; graph and text views | Accepted | 2026-10-13 |
| [0058](0058-turning-v2-defaults.md) | Turning v2 defaults: the numbers, RCS couples and when engines help turning | Accepted (Niklas, 17 Oct 2026); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0059](0059-behavior-editor-wave-a-defaults.md) | Behavior editor wave A defaults: editing rules, storage and tests, the screen | Accepted (Niklas, 17 Oct 2026); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0060](0060-behavior-editor-wave-b-defaults.md) | Behavior editor wave B defaults: editing gestures, saving and linking, the live panel | Accepted (Niklas, 17 Oct 2026); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0061](0061-fight-balance-after-turning-v2.md) | Fight balance after turning v2: raiders orbit inside the beams, the skirmisher keeps its nose on the target, shields can collapse | Accepted | 2026-10-13 |
| [0062](0062-first-playable-loop.md) | The first playable loop: start at home, short hand-made rim voyages, mind capacity 2 → 4 | Accepted | 2026-10-13 |
| [0064](0064-lock-feed-forward-and-ai-flight-fixes.md) | The lock and `face` lead a moving target's turn; AI flight fixes from the balance study | Accepted | 2026-10-13 |
| [0063](0063-salvage-and-research-v1.md) | Salvage and research v1: fly over wrecks, the haul is dismantled at home into research data, unlocks are unlimited | Accepted | 2026-10-13 |
| [0065](0065-phase-7-screens-and-escort-action.md) | Phase 7 screens: debrief panel, research tree, star map, recall under the cargo, fleet formation, locked parts shown dimmed; an `escort` action | Accepted; the debrief's one-line summary replaced by the LOG ([GDR-0091](0091-log-building-and-voyage-history.md)) | 2026-10-13 |
| [0066](0066-phase-7-wave-1-defaults.md) | Phase 7 wave 1 defaults: salvage values and fragments, cargo and collecting, the career model, the home screen | Accepted (Niklas, 17 Oct 2026); vanishing cargo superseded by [GDR-0076](0076-spilled-cargo-becomes-fragments.md); collecting speed and loose-item lifetime by [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0067](0067-rams-pass-friendly-bubbles.md) | Rams pass friendly shield bubbles, like shots and plumes | Accepted | 2026-10-13 |
| [0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md) | A voyage site clears when its enemies are destroyed or disarmed; only armed enemies block recall | Accepted | 2026-10-13 |
| [0069](0069-machines-drawn-as-components.md) | Machines are drawn as their own components; engine nozzles reach past the hull | Accepted | 2026-10-13 |
| [0070](0070-phase-7-wave-2-defaults.md) | Phase 7 wave 2 defaults: flight and AI numbers, voyage clearing details, the career save, the drydock gating | Accepted (Niklas, 17 Oct 2026); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md) | 2026-10-13 |
| [0071](0071-shipyard-home-long-u.md) | The home screen is the shipyard: a long U-shaped dock, the building area fills the U, the gate at the open side | Accepted | 2026-10-13 |
| [0072](0072-shipyard-parts-voyage-flight-defaults.md) | Defaults of the shipyard home, the machine drawings and voyage flight | Accepted (Niklas, 17 Oct 2026); abandon superseded by [GDR-0077](0077-abandon-voyage-asks-in-a-panel.md); reviewed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md); "V flies the first open voyage" superseded by [GDR-0097](0097-galaxy-map-and-voyage-flow.md) | 2026-10-13 |
| [0073](0073-slice-tuning-v1.md) | Slice tuning v1: smaller packs, a gentler first voyage, research costs about halved | Accepted; 2026-10-17 re-measured, the Warden's raiders 2–3 and Corran Reach's hunter packs ×2 | 2026-10-13; 2026-10-17 |
| [0074](0074-test-range-waves-emergency-recall-cannon.md) | Test-range waves follow the flown ship; recall also works when the fleet is disarmed; the cannon becomes an upgrade | Accepted | 2026-10-13 |
| [0075](0075-voyages-feel-like-exploration.md) | Voyages should feel like exploring a galaxy and following hints, not a fixed path (direction) | Accepted; first design [GDR-0078](0078-galaxy-exploration-v1.md) | 2026-10-14 |
| [0076](0076-spilled-cargo-becomes-fragments.md) | Cargo that no longer fits a shrinking hold spills as loose items that can be collected again | Accepted | 2026-10-14 |
| [0077](0077-abandon-voyage-asks-in-a-panel.md) | Abandoning a voyage asks in a panel (M opens it) instead of M twice | Accepted | 2026-10-14 |
| [0078](0078-galaxy-exploration-v1.md) | Galaxy exploration v1: signals that narrow as you search, blind jumps, jump range instead of lanes, hidden gates, scanning inside clusters | Accepted; jumps from any charted cluster in [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md) | 2026-10-14 |
| [0079](0079-review-of-lead-defaults-14-oct.md) | Review of the lead defaults: most confirmed; collecting up to 8 m/s, loose items last the voyage, a speed zoom in flight | Accepted | 2026-10-14 |
| [0080](0080-exploration-searching-and-story.md) | Exploration details: 2–3 km clusters, a scanner part, scans wake enemies, about two steps to locate, free decoding, blind jumps sometimes start a trail, story by fragments and milestone cards | Accepted | 2026-10-14 |
| [0081](0081-drydock-editing-bevel-rotate-replace.md) | Drydock editing: bevel after the stroke, R / Shift+wheel turn the brush (slopes auto first), placing and painting replace what is under them, shield parts show their radius while placing | Accepted | 2026-10-14 |
| [0082](0082-drydock-layout-v2-and-part-availability.md) | Drydock layout v2: every part in sections with scrolling jump chips, an armour swatch strip on the left, tools in the bottom bar (a dropdown when narrow), icons and a ⋯ menu; part availability tracked, unlimited for now | Accepted | 2026-10-14 |
| [0083](0083-evolution-lab.md) | An evolution lab: configurable evolution of designs and behaviors (fitness and limits per run), a dev-only shipyard place first, a benchmark rating for players later | Accepted; superseded in part by [GDR-0092](0092-evolution-lab-for-your-own-ships.md) | 2026-10-14 |
| [0084](0084-chain-reactions-v1.md) | Chain reactions v1: destroyed reactors, capacitors and drives explode into their piece (content blast numbers, armour takes a share, same tick, own shields no help) | Accepted (Niklas, 17 Oct 2026); reach, armour shielding, radii and more exploders by [GDR-0085](0085-chain-reactions-v2.md) | 2026-10-14 |
| [0085](0085-chain-reactions-v2.md) | Chain reactions v2: blasts reach every piece and ship (other ships' shields absorb, friends are hit), intact armour on the line lets only its share through, reactors reach two cells deep, automaton cores, shield emitters and RCS pods explode; reserve guns stay | Accepted (rules Niklas; numbers and details accepted by Niklas 17 Oct 2026) | 2026-10-14 |
| [0086](0086-letting-go-coasts.md) | Letting go coasts: no braking and no flip-and-burn on release (keys and stick); auto-braking to come back later as a configurable assist | Accepted | 2026-10-14 |
| [0087](0087-voyage-transitions.md) | Voyage transitions: the fleet visibly warps out (charge, stretch, flash) and in, the camera eases in 12 % during the charge, the emergency recall looks unstable, every end dips to dark and the next screen comes up out of it | Accepted (Niklas, 17 Oct 2026) | 2026-10-14 |
| [0088](0088-pilot-assists-weapon-groups-and-key-bindings.md) | Pilot assists as rules per design (fill idle channels, switchable in flight by key and condition), weapon groups 1–3 set in the drydock, an action map with a settings screen | Accepted; lead defaults accepted by Niklas 17 Oct 2026 | 2026-10-14 |
| [0089](0089-play-test-15-oct-speed-salvage-waves-station.md) | Play-test of 15 Oct: at the top speed the velocity follows the push (the fade weighted by the push angle, a governor on the ship's own thrust), salvage settles to a slow drift, test-range waves bring the biggest enemies first, the station comes alive | Accepted (Niklas, 17 Oct 2026), except the speed governor (provisional, lead: judge in play) | 2026-10-15 |
| [0090](0090-action-map-settings-and-pause-menu.md) | The action map (every key an action with up to two inputs, defaults in `controls.ron`), SETTINGS with controls and display over home and flight, Esc in flight unlocks else opens the flight menu (pauses the test range only), assist toggle slots | Accepted | 2026-10-15 |
| [0091](0091-log-building-and-voyage-history.md) | The LOG: a records mast by the gate that glows while a voyage's result is new, its tooltip the last voyage, its screen the last 50 voyages newest first; it replaces the one-line summary over home | Accepted | 2026-10-15 |
| [0092](0092-evolution-lab-for-your-own-ships.md) | The evolution lab is a station building for improving your own ships: runs from your designs, locks, combinable trials with co-evolving enemies, everything mutable, every fight re-watchable from a fight code, results saved as designs | Accepted, step 1 built (the fight viewer) | 2026-10-15 |
| [0093](0093-galaxy-exploration-decisions-16-oct.md) | Galaxy exploration, 16 Oct: the S unit, a light scanner on the starter skiff, jump range 2.5 → 4 S from home or any charted cluster, how a signal looks and reads, SCAN for pilots and behaviors, data cores in the hold | Accepted | 2026-10-16 |
| [0094](0094-galaxy-generator-defaults.md) | The galaxy generator's defaults: clusters 0.9 S apart fill five ring bands (about 80, most in the rim), 3 hidden gates per band, trails walked towards home, the rim real and the rest stubs | Accepted (Niklas, 17 Oct 2026) | 2026-10-16 |
| [0095](0095-sensors-hidden-sites-and-scans.md) | Sensors, hidden sites and scans: cores see 800 m (behaviors' targets too), signals' sites show within 400 m or under a sweep, far ones get exact bearings, the `scan` action steps aside, the cluster's edge turns ships back, the mirror lock keeps a single part in place | Accepted (Niklas, 17 Oct 2026) | 2026-10-16 |
| [0096](0096-career-galaxy-format-3.md) | The galaxy in the career: save format 3 keeps the skeleton, charting on arrival, signals that halve towards their place, data cores as one-slot logs, Jump calibration I and II (2.5 → 4 S), the slice pinned in a chain from home; the drydock's power leaves bursts apart | Accepted (Niklas, 17 Oct 2026) | 2026-10-16 |
| [0097](0097-galaxy-map-and-voyage-flow.md) | The galaxy map and the voyage flow: the map on the career's skeleton, signals as gold areas, soft range circles, blind jumps to the nearest cluster, the warp on the map, finds kept on a lost voyage, decoding at the LOG, a cluster map in flight; the map's backdrop a tenth as heavy | Accepted (Niklas, 17 Oct 2026) | 2026-10-16 |
| [0098](0098-slice-in-the-galaxy-and-rim-rewards.md) | The slice in the galaxy and the rim's rewards: set pieces by content, the Warden's gate the rim's first gate, signal areas 3–4 S narrowing to 60 %, four free clusters by home, caches, memories and graveyards; save format 4 | Accepted (Niklas, 17 Oct 2026) | 2026-10-17 |
| [0099](0099-the-grown-and-the-outer-reach.md) | The Grown and the outer reach: heart-fed regrowth, rammers with thorn guns, calm until provoked, regrowing armour and a ram prow first, hot clusters, a precursor guardian at each gate inward | Accepted (Niklas, 17 Oct 2026); phase 1 (mechanics and parts) built, phase 2 not yet | 2026-10-17 |
| [0100](0100-fight-balance-targets.md) | Fight balance targets: each slice voyage harder than the last, a Runner and an escort clear the Warden about 40 %, shields collapse under sustained fire, encounter counts tuned first | Accepted (Niklas, 17 Oct 2026); B2 since capacity 2 (17 Oct: one wave in six) | 2026-10-17 |
| [0101](0101-the-automaton-gunner.md) | The first voyages' enemy is the automaton gunner: twin cannons, aimed at the engines | Accepted (Niklas, 17 Oct 2026: a different first enemy); its numbers lead defaults | 2026-10-17 |
| [0102](0102-grown-mechanics-and-parts-defaults.md) | The Grown's mechanics and parts: one regrown part every 2.5 s from a working heart, tissue that heals, a prow that deals twice and takes half, thorn guns, wary sites that wake on provocation and stripped wrecks, hot stars by the inverse square | Accepted (provisional, lead) | 2026-10-17 |
| [0103](0103-the-grown-in-play-defaults.md) | The Grown in play: rammers, thornbacks, grove guardians and elders on a ganglion beside the heart, prows that charge from thorn range, small groves, a bastion at each outer gate, elders at the end of their own trail, one Grown research chain | Accepted (provisional, lead) | 2026-10-18 |
| [0104](0104-scenery-rocks-filler-landmarks.md) | Scenery between the sites: anchored asteroid rock as bodies placed by the cluster's scenery (fields, belts, scattered; rams scaled by the rock, behaviors steer round it), drawn-only dust, debris, derelicts, landmarks, a hot star's corona and distant fights | Accepted (provisional, lead) | 2026-10-17 |
| [0105](0105-cluster-sky-planets-nebula-galaxy.md) | A cluster's sky: a planet (13 kinds), its moons (6 kinds, never two alike), a nebula and a far galaxy in soft vertex-colour style B, rolled from the cluster's seed, kinds and skies in content, drawn only at 85 % | Accepted (provisional, lead; style Niklas's) | 2026-10-17 |
<!-- index:end -->

## How to add, supersede and deprecate

- **Add:** copy [`0000-template.md`](0000-template.md) to the next free number (`NNNN-short-slug.md`), keep it readable in under two minutes, add its row above and link it from `docs/concept-notes.md`. Agents write `Proposed`; Niklas's decisions are `Accepted`; lead-session defaults are `Accepted (provisional, lead)`.
- **Supersede:** never rewrite a decision. Write a new record with a `Supersedes: GDR-NNNN` line, then change only the old record's status to `Superseded by GDR-MMMM` (and its row here).
- **Deprecate:** when a decision stops applying and nothing replaces it, set its status to `Deprecated` and add one dated line saying why.

## Open

Not decided yet, so no record. Write one when Niklas decides.

- **The cannon starter** (`docs/results/slice-balance-2026-10-17b.md`, [GDR-0100](0100-fight-balance-targets.md)): the starter with the reverse-engineered cannon alone clears the Warden 37 %, level with a Runner and an escort (42 %); its curve 93 / 68 / 46 / 37 %. Niklas decides after seeing it (part numbers or the research node).
- **Feel of "fly that way"** for forward-only ships: let the nose swing past the stick ("drift turn", current behaviour, recommended) or keep the nose on the stick (`docs/sim.md` design question 3).
- **Mind capacity numbers** beyond the slice (the slice: start 2, research to 4, [GDR-0062](0062-first-playable-loop.md); the exclusive resource) and N parts per core type ([GDR-0013](0013-vessels-and-piloting.md), [GDR-0038](0038-logic-cores-control-parts.md), [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md)); lead placeholders until then.
- **Engine plume** numbers and the v1 defaults ([GDR-0049](0049-engine-plume-v1.md): damage and heat scale with output, blocked plumes keep thrust, enemy bubbles absorb plumes) (`docs/sim.md` design question 16). No push and the faint reach are decided ([GDR-0055](0055-slower-crisper-turning.md)).
- **Other research sources** besides salvage and scans ([GDR-0025](0025-research-from-salvage.md)).
- **Tree size per family** that stays fun instead of tedious ([GDR-0024](0024-tech-trees-no-mk-levels.md)).
- **Overdrive** details ([GDR-0004](0004-grid-ship-building-no-wiring.md)); **missiles, disruption and drones** are not built yet ([GDR-0018](0018-weapon-families.md)).
- **New game+** (what carries over) and the **working title**.

Parked (maybe later): location-based play (light version: your real location seeds your home region), co-op
raids (after base co-op works), shared blueprints + behavior trees as AI opponents (bounty hunters, async PvP).
