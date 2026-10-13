# GDR-0054: Drydock: enemy parts locked until unlocked; fill and bevel confirmed; the working design autosaves; designs are stored and can be shared

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively)
- Supersedes: the palette rule of [GDR-0053](0053-drydock-v1-lead-defaults.md) ("every part and armour material is offered")
- Related: [GDR-0051](0051-drydock-v1.md), [GDR-0053](0053-drydock-v1-lead-defaults.md), [GDR-0025](0025-research-from-salvage.md), [GDR-0027](0027-races.md), [ADR-0024](../adr/0024-saves-seed-and-deltas.md), [ADR-0030](../adr/0030-drydock-architecture.md), `docs/drydock.md`

## Context

Wave 1 of the drydock offered every part, the automaton core, cannon and plate included, and set lead defaults
for fill (a dragged rectangle) and bevel (a mode). Wave 2 adds saving, so the drydock needs to know what it
starts with and how designs are kept. Niklas also noted that, long term, designs and the rest of the player's
data obviously need proper storage.

## Options considered

- Enemy parts: **hide until unlocked** (chosen) or show everything in v1.
- Fill: **drag a rectangle** (chosen), a bucket fill, or both.
- Bevel: **a mode while painting** (chosen), a one-shot command, or both.
- Start design: **the last design, autosaved** (chosen), always a Runner copy, or an empty frame.
- Storage: **a player storage layer** (chosen) for designs now and save games, behaviors and settings later; or
  designs written into a local player pack; or both. Niklas added: **plus a sharable format, to share a design
  with others**.

## Decision

- Parts and materials the player hasn't unlocked are hidden from the drydock's palette and armour choice. In v1
  that means the automaton parts and Automaton plate; salvage and research unlock things later. Content ships
  that use locked parts still open and fly. Which entries are locked is content data, not code.
- Fill is a dragged rectangle; bevel is a mode while painting (GDR-0053's defaults, now confirmed).
- The drydock keeps the working design across reloads (autosave); the very first start is a copy of the Halvex
  Runner; content ships open as copies from the blueprints list.
- Designs live in a player storage layer, which later also holds save games, behaviors and settings.
- A design can be shared: exported as a file in the content's ship format and as a copy-paste share code, and
  imported from either.

## Consequences

- Parts and materials get a "locked" flag in content; an unlock system (salvage, research) will clear it per
  player later.
- The storage layer's technical shape (backends per platform, document versions) is an ADR.
- A shared design names the content it needs, so a receiver without a mod gets a clear message instead of a
  broken ship.
