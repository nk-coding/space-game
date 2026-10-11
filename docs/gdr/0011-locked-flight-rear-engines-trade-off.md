# GDR-0011: Ships with rear-only engines can't brake or strafe while locked: a design trade-off

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Related: [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0010](0010-turning-sources.md), `docs/sim.md` "Design questions for Niklas" (question 2)

## Context

Under a lock the flight computer keeps the nose on the target (GDR-0009). A ship whose engines all point backwards
then has nothing that pushes sideways or forwards, so it can neither strafe nor brake.

## Options considered

- **(a) Accept it; RCS pods or side and front thrusters are the answer**, a choice made in the drydock
  (recommended by the lead; chosen).
- **(b) Let the flight computer break the lock briefly** to flip and brake.

## Decision

Option (a). Strafing or braking while locked needs RCS pods or side/front thrusters; rear-only ships must turn
around.

## Consequences

- RCS pods and side thrusters have a clear job, which gives the drydock a real trade-off.
- The flight computer never breaks a lock on its own.
