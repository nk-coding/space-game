# GDR-0006: Ships are ~60×60 blocks and up, with no fixed block cap

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0004](../adr/0004-target-platforms-and-performance-first.md), [ADR-0012](../adr/0012-coordinate-frames-and-units.md), [GDR-0005](0005-fully-physical-ships.md), [GDR-0013](0013-vessels-and-piloting.md)

## Context

Physical ships made of 1 m blocks (ADR-0012), on mid-range phones. Players should be able to grow from small
ships to capital ships.

## Options considered

- **A fixed block cap**: rejected in favour of a measured, per-device limit. Reason not recorded beyond that.

## Decision

- Ships of at least ~60×60 blocks (60 m), capital ships if performance allows.
- No fixed block cap: the drydock measures a design's simulation cost and warns when it is too heavy for the
  current device. Merged colliders and level of detail keep big hulls cheap.
- Ship size grows with drydock expansions (frigate, cruiser and capital frames) and with mind capacity.

## Consequences

- The drydock needs a cost estimate per device (device load is one of its live stats).
- Note: on 10 Oct mind capacity meant "how many parts the ship-mind controls", raised by logic cores and research.
  On 11 Oct GDR-0013 defined it as the number of logic cores the mind can run at once. Whether it still limits
  ship size is open.
