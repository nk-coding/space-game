# GDR-0035: Rule slots: six per logic core, two per automaton core; the lowest rules switch off first

- Status: Accepted
- Date: 2026-10-11 (proposal); 2026-10-12 (confirmed by Niklas)
- Decided by: Niklas (confirmed the proposal: 6/2 slots, lowest rules drop first); behavior agent (proposal)
- Related: [GDR-0015](0015-layered-behavior.md), [GDR-0013](0013-vessels-and-piloting.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md), `docs/behavior-format.md`, `docs/content-model.md` "Behaviors"

## Context

GDR-0015 says rule slots come from logic cores. Behavior v1 needed the numbers and what happens when a core is
lost mid-fight or a behavior is longer than the ship's slots.

## Options considered

- **Numbers**: the concept-board drydock gives a logic core 6 slots; the automaton core (content, invented) 2.
  Kept as placeholders in content (`logic.rule_slots`).
- **Over the limit**: (a) refuse to run the behavior; (b) **run the first N rules, the rest are inactive**
  (chosen): a damaged ship degrades gracefully and the player sees which rules dropped out; (c) let the player
  choose which rules survive: more UI, unclear in a fight.
- **Content over the limit**: (a) warning; (b) **error** (chosen): a base or mod ship whose (faction default)
  behavior has more rules than its cores give is almost certainly a mistake.

## Decision

- Slots = sum of `rule_slots` of the **working** (alive, not overheated, powered) logic cores of the piece that
  runs the behavior. Recomputed at every think.
- Rules beyond the slots are inactive (debug view: "no slot"); losing a core switches off the lowest rules first.
- Validation: a ship's effective behavior must fit the slots of its intact cores (a ship without cores is not
  checked: it cannot think).
- Placeholders: logic core 6, automaton core 2. The raider pack fits in two rules.
- Niklas (12 Oct) confirmed the numbers and that the lowest rules drop first (not a player-chosen order).

## Consequences

- Fits vessels (GDR-0013): the limit is per piece, so a split-off vessel runs the inherited behavior within its
  own cores' slots (`Ship::rule_slots_of`). Dormant cores (mind capacity) will simply not count as working.
- Order in the list is also a survival order: players put must-keep rules (flee, defend) on top.
