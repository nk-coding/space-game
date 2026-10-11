# GDR-0003: You pilot one ship with a small scripted escort; the fleet is secondary

- Status: Superseded by [GDR-0013](0013-vessels-and-piloting.md)
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0015](0015-layered-behavior.md), [GDR-0017](0017-combat-support-repair-capture-sensors.md), GDR-0027

## Context

The brief asks for ships you build and fly yourself, plus default navigation and deep behavior customisation. The
concept board's heading for this: "Your ship comes first".

## Options considered

Not recorded.

## Decision

As decided in the concept phase:

- You pilot one ship.
- The fleet is secondary: escorts are built at home within your **command capacity**, drones launch from hangar
  parts, and escorts run behavior trees (scripted, GDR-0015).
- Capture of disabled ships and allies in race missions belong to the same concept row; they live on in GDR-0017
  and GDR-0027.

## Consequences

Superseded on 11 Oct 2026 by GDR-0013 (vessels and piloting). What carries over: one ship piloted directly, the
rest run their behavior, the piloted ship comes first. What changed: every piece with a working logic core is a
vessel, focus can switch instantly to any own vessel, split-off cored pieces become vessels, and command capacity
is replaced by mind capacity (the number of running logic cores).
