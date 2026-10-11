# GDR-0037: The behavior vocabulary is content; v1 has 24 nodes, one target line with aim priorities, and the raider pack as data

- Status: Accepted
- Date: 2026-10-11 (proposal); 2026-10-12 (accepted; Niklas's answers below)
- Decided by: Niklas (one target line per behavior; `skirmisher` for unpiloted ship-mind vessels; delete the raider brain); behavior agent (vocabulary as content, the v1 nodes; proposal accepted 2026-10-12)
- Related: [GDR-0015](0015-layered-behavior.md), [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md), [ADR-0016](../adr/0016-wiki-ready-content-model.md), [ADR-0026](../adr/0026-behavior-programs-text-in-content.md), `docs/content-model.md` "Behavior nodes", `docs/behavior-format.md`

## Context

The editor palette, the wiki and modders all need the list of conditions, actions and target selectors with
names, descriptions and parameter ranges. Enemy designs ship with behaviors that scanning reveals later, so
behaviors must be data that can be shown. The combat notes ask for player-set target priorities (engines
first, weapons first, nearest threat).

## Options considered

- **Vocabulary in code** (names and help text in Rust): the wiki and editor would need a second copy; rejected.
- **Vocabulary as content** (chosen): one entry per node in `behavior_nodes/` with typed parameters, defaults,
  ranges and text; the sim implements each built-in id and checks both directions at load.
- **Target priority**: (a) a ship-level setting outside behaviors; (b) **part of the `target` line**
  (chosen): `target nearest(engines)`, so presets and enemy behaviors carry their priorities and scans show them;
  (c) per-rule targets: more power, more clutter (later, if needed).

## Decision

- Conditions: `hull`, `shield`, `shield_down`, `heat`, `power`, `target_in_range`, `target_hull`,
  `target_on_me`, `under_attack`, `enemies_near`, `cycle`, `target_time`, and the Rhai condition `closing_in`.
- Actions (maneuvers): `approach`, `orbit`, `kite` (keep distance), `face`, `flee`, `hold`. `ram` waits for
  collisions.
- Target selectors: `nearest`, `weakest`, `threat`, `attacker`, `locked` (the leader's lock, e.g. the player's
  for escorts), each aiming at `centre`, `engines`, `weapons`, `power` or `core`.
- One target line per behavior (Niklas, 12 Oct): no per-rule targets until a preset needs them.
- Behaviors are content: `raider_basic` (the automatons' default: the old hard-coded raider brain as two rules)
  and `skirmisher` (the ship-mind's default for unpiloted vessels, confirmed by Niklas on 12 Oct: flees below
  25% hull, dodges while its shields recharge). Factions name a default behavior; ships may
  name their own.

## Consequences

- Mods add conditions as Rhai scripts; new built-in actions need code plus a content entry (the load check
  catches a missing half).
- The content raider fights like the old brain (same first shot within 2 ticks, 74 vs 74 shots in a 40 s test
  fight). The brain (`RaiderBrain`, `ShipCommand::Brain`) was deleted on 12 Oct (Niklas; ADR-0025) once the
  client, the load test and the scenarios used behaviors.
- A future "set power tier" action (GDR-0014) fits as another action node.
