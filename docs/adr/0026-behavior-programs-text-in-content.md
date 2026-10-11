# ADR-0026: Behavior programs are text lines in RON, parsed in `data`, compiled once and evaluated from one snapshot without allocation

- Status: Accepted
- Date: 2026-10-11 (proposal); 2026-10-12 (accepted with Niklas's answers on GDR-0034 to GDR-0037)
- Decided by: behavior agent (proposal, accepted 2026-10-12); Niklas (delete the raider brain)
- Related: [ADR-0010](0010-cross-platform-determinism.md), [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0027](0027-rhai-sandbox-and-determinism.md), [GDR-0015](../gdr/0015-layered-behavior.md), [GDR-0034](../gdr/0034-behavior-first-match-fire-at-will.md), [GDR-0036](../gdr/0036-behavior-text-format.md), [GDR-0037](../gdr/0037-behavior-vocabulary-as-content.md), `docs/sim.md` "Behaviors", `docs/content-model.md`

## Context

Behaviors need a text form (editor, wiki, scans), a content form (RON packs, mods) and a fast deterministic
evaluation in `sim` for many ships per tick.

## Options considered

- **Content form**: (a) structured RON (`(when: [(cond: "hull", ...)], then: ...)`): verbose, a second
  representation to convert and keep in sync; (b) **the text lines themselves** (chosen): `target: "nearest"`,
  `rules: ["when hull < 30% -> flee", ...]`, one parser and printer, errors point at `rules[i]` and a column.
- **Where the parser lives**: **`data`** (chosen; content validation, the wiki tool and the editor need it
  without the sim) vs `sim`.
- **Evaluation**: interpret the AST each tick vs **compile once to flat enums** (chosen): names, defaults and units
  are resolved at load; a think reads numbers only.

## Decision

- `data::behavior`: lexer, parser, canonical printer, vocabulary check (`check_program`), typed values.
- `sim::behavior`: `BehaviorLibrary` (vocabulary checked against `BUILTIN_NODES` both ways, programs compiled),
  `ShipCommand::Behavior(Option<BehaviorId>)` / `Leader`, a think pass between breakage and flight.
- The think pass reads one snapshot of all ships, writes per-ship results to scratch, then applies them (ship
  order never matters); per-pass caches (hull shares, ship centres) and swapped status buffers: no allocation
  per think (tested).
- State hash includes each ship's behavior state and the program's FNV fingerprint (two peers running different
  programs diverge visibly).

## Consequences

- Cost (native release, VM load ~10, 20 ships × 10 rules): a think pass over all 20 ships ~10–17 µs (min /
  median), about 0.5–0.8 µs per ship; with the 3-tick cadence ~4–6 µs per tick.
- The vessel split (GDR-0013) copies a ship's `BehaviorState::inherit()`: the program is shared by `Arc`.
- Fire at will per gun (GDR-0034) lives in the flight step: the plan carries the target's state
  (`Fire::AtWill`), `behavior::guns_at_will` fills the per-slot mask `ShipControls::guns`, and weapon power
  demand reads `ShipControls::gun_fires(slot)`. No change in the weapons code.
- `RaiderBrain` and `ShipCommand::Brain` were deleted on 12 Oct (Niklas; ADR-0025): scenarios, the load test,
  tests and the client use behaviors.
