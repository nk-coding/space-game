# ADR-0030: Drydock architecture: a pure editing model in `data`, a static design report in `sim`, player designs as content blueprints

- Status: Accepted (provisional, lead)
- Date: 2026-10-12
- Decided by: lead session (default for Niklas to review)
- Related: [GDR-0051](../gdr/0051-drydock-v1.md), [ADR-0003](0003-crate-architecture.md), [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0024](0024-saves-seed-and-deltas.md), [ADR-0025](0025-main-repo-holds-only-chosen-code.md), `docs/drydock.md`, `docs/sim.md` "Design report", `docs/client.md` "Drydock"

## Context

The drydock (GDR-0051) edits a ship block by block and shows live numbers after every edit: power, peak heat,
thrust over mass, top speed, turning, firepower, control load, device load, plus overlays for heat, mass, bonus
links, shields and gun and plume arcs. Those numbers come from the sim's rules (power allocation, heat, bonuses,
thrust allocation, rated speed); a second copy of the rules in the client would drift. Before this, the sim
derived them only on a spawned `Ship`, which needs a `Sim` and a Rapier body, and nothing computed a steady-state
heat. The data crate could validate only a whole content set, and nothing wrote a blueprint back to RON. The
client cached designs and visuals by content ship id, so an in-memory blueprint could not be flown. The arena's
evolution operators (later) need the same blueprint edits as the drydock.

## Options considered

Editing logic:
- **In the Bevy client**: quick to wire, but untestable without a window and not reusable by the arena.
- **A pure module `data::edit`** (chosen): placement legality, paint, fill, erase, bevel, slope orientation,
  mirror and undo/redo on a `data::Blueprint`, with unit tests and no engine.
- A new crate: one more crate for a module that only needs `data`.

Live stats:
- **Spawn the design into a throwaway `Sim`** after every edit: reuses the rules, but drags Rapier in, and the
  steady-state heat still needs its own solver.
- **A copy of the rules in the client** (the concept board's `drydock-model.js` approach): drifts.
- **A static design report in `sim` (`sim::report`)** (chosen): computed from the blueprint with the same code the
  `Ship` uses (shared, not copied), no world, recomputed after every edit; tests compare it against a spawned
  `Ship` for every base ship so the two cannot drift. Structured data only; the client formats the text.

Player designs:
- **The content ship schema** (chosen): a saved or exported design is a `Blueprint` written as RON by `data`, so
  it loads like content, can be validated with `data::validate_ship`, and can later move into a mod pack.
- A separate save format: two formats to migrate.

## Decision

- `data` gains `validate_ship` (one blueprint, the same checks as content validation), a RON writer and reader
  for blueprints, drydock frames as a content kind (GDR-0006) and ship totals, and the pure editing model
  `data::edit`.
- `sim` gains `sim::report`: the design report, including a steady-state heat solve and a device-load estimate.
  Refactors that share code between `Ship` and the report must leave every state hash unchanged.
- The client adds a `Drydock` app state next to flight, draws the design with the flight tile pipeline (the same
  tiles everywhere, `docs/style-guide.md`), keys its design and visual caches per design instead of per content
  id, and routes every button and shortcut through one action message.
- Player designs are stored as RON blueprints: in the browser's local storage on the web build, as files on
  native builds; export writes the same RON.

## Consequences

- The drydock's rules live in exactly one place each: geometry and legality in `data`, ship behaviour in `sim`.
- The arena can reuse `data::edit` for mutations and `sim::report` for cheap fitness pre-checks.
- Saved designs depend on content ids staying stable (already a rule, `docs/content-model.md`).
- Browser local storage is per origin: designs saved at `10.77.9.2:8080` are not visible at `:8081`. Export is
  the way to move a design.
