# GDR-0123: Behavior words for salvage: a `salvage` action, `wreck_in_range` and `cargo_full`, and a `salvager` escort template

- Status: Accepted (provisional, lead), 20 Oct 2026; the escort choice confirmed by Niklas, 20 Oct 2026.
  2026-10-20 (Niklas): the voyage runner's escorts keep `escort`; `salvager` stays the player's choice until a tractor
  exists.
- Date: 2026-10-20
- Decided by: lead session (`docs/plans/first-playable-loop.md` §7 "Behavior words for salvage"; every number a lead
  default)
- Related: [GDR-0063](0063-salvage-and-research-v1.md) (collecting by flying slowly over salvage), [GDR-0065](0065-phase-7-screens-and-escort-action.md)
  (escorts and their template), [GDR-0114](0114-mining-and-ore-refining.md) (the `mine` action and the `miner`
  template, the pattern), [GDR-0031](0031-materials-and-fabrication.md) (the tractor, still open), `docs/behavior-format.md`

## Context

The first playable loop's plan left salvage words out of the slice (§7: `wreck in range`, `cargo full`, `tractor`;
escorts did not need to salvage). Since GDR-0114 a ship can mine between fights with a content behavior, but only
the voyage runner's own pilot strips wrecks; an escort with a hold collects only what it happens to fly over. There
is still no tractor (GDR-0031), so collecting stays GDR-0063's: a vessel takes from salvage within reach of its hull
that drifts slowly relative to it.

## Options considered

- **What the action flies**: to the nearest salvage within sensor range that drifts slower than 6 m/s and holds
  something that fits the free hold (a loose item, a wreck's own cargo or an alive salvageable part), onto the piece
  at range 0 (or to the loose item), so the hull settles over it and collecting does the rest (chosen: the same
  rule as `mine`, the same reach as sensors, and nothing new for collecting); the runner pilot's value-per-unit
  ranking with give-up timers (not chosen: a behavior keeps no memory between thinks, and nearest is easy to read).
- **`wreck_in_range`**: a flag with a distance (default 250 m, the runner's strip range) that measures to the nearest
  such salvage whatever the hold (chosen: the hold has its own word, so the two combine:
  `when wreck_in_range(200m) and not cargo_full -> salvage`); salvage that fits the hold only (not chosen: two words
  that mean the same half of a rule); a count of wrecks (not chosen: nothing in the plan needs how many).
- **`cargo_full`**: a flag, true when no unit is free (a ship without a hold is always full) (chosen); a `cargo`
  percentage (not chosen for now: nothing needs "nearly full" yet; easy to add).
- **The voyage runner's escorts**: escorts with a hold fly `salvager` the way escorts with a mining laser fly `miner`
  (not chosen: measured, see below, it costs goals, and no part says a design is meant to strip, unlike the mining
  laser); escorts keep `escort` (chosen; `salvager` is for players, and for the runner once a tractor or another
  salvage part can say so).
- **Fire while salvaging**: hold fire (not chosen: an escort over a wreck would ignore the leader's target); fire at
  will at the target as every maneuver does (chosen).

## Decision

- **`salvage`** ("Salvage nearest wreck", an action that needs no target ship, like `mine`): the nearest salvage
  within sensor range (from the ship's centre to a salvage piece's centre of mass or a loose item) drifting no faster
  than `behavior::tuning::SALVAGE_DRIFT` (6 m/s, the runner's `CATCHABLE`) with something that fits the free hold;
  ties loose items first, then ship and piece order. It flies `Approach { target: Piece, range: 0 }` onto a piece
  or `Hold` at a loose item; guns fire at will at the target. With a full hold or nothing that fits it steps aside
  like a rule without a target. Only owners that collect (the player) take anything; an enemy flying it only goes
  there.
- **`wreck_in_range(range)`** (default 250 m): the same search with any hold; holds when the nearest is within the
  range (the debug view shows its distance); false with no salvage in sensor range, so `not wreck_in_range` holds
  there.
- **`cargo_full`**: no free hold unit.
- **The `salvager` behavior** (content): the escort's four rules with salvage between fighting and keeping near:
  `when hull < 30% and target_on_me -> kite(150%)`, `always -> approach(70%)`,
  `when wreck_in_range(250m) and not cargo_full -> salvage`, `always -> escort(30m)`, target `locked(core)`.
- **The voyage runner and the arena's voyage fitness** keep `escort` for escorts with a hold. Measured (80 seeds,
  rim skiff with two rim skiff escorts, all with `salvager` against `escort`): `yard_approach` goal cleared and home
  47 against 61, haul items median 8 against 5; `halvex_drift` 37 against 45, haul 8 against 4. Escorts stripping
  wrecks fall behind the flagship and reach the next site late.
- The salvage determinism scenario has one ship under `salvager`.

## Consequences

- A player can give escorts with cargo bays `salvager` and get about double the haul from a voyage at the cost of
  goals; the trade is theirs to make.
- New vocabulary changes the content fingerprint (saved lab runs restore stale) and the evolution's mutation draws
  (the tiny evolve test moved to seed 2). Behaviors that never salvage hash as before (the choice lives in the
  maneuver, already hashed).
- Open: the tractor (GDR-0031) and with it a part that makes the runner's escorts salvage; a word for the distance to
  the leader, so a salvager does not fall behind; a `cargo` percentage; salvage in the evolution lab's trials.
