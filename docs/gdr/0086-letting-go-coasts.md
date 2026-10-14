# GDR-0086: Letting go coasts: no braking and no flip-and-burn on release

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas
- Supersedes: GDR-0009 (only "letting go brakes"), GDR-0040 (only "brakes when the keys are released")
- Related: [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0015](0015-layered-behavior.md), `docs/sim.md` "Flight", `docs/client.md` "Controls"

## Context

Since GDR-0009 the flight computer braked to a full stop whenever the player let go of the stick or the keys. A
ship whose engines only push forward can only do that by turning 180° and burning (flip-and-burn), so every
release spun the ship around. Niklas, 14 Oct 2026: "the automatic 'rotate ship and slow down' is really annoying,
this should not be the default". Asked what letting go should do instead: "imho this behavior should be
configurable (in general ship automation should be configurable via this behavior stuff, there should be a way to
automate not only autonomous vehicles, but also assists for the main ships, as well as bindings …) either way for
now just completely get rid of that weird braking behavior".

## Options considered

- **Brake without turning, S flips**: keeps a stop on release where the ship can push against its motion. Not
  chosen: Niklas wants release-braking gone entirely for now.
- **Coast; braking becomes a configurable assist later** (chosen).

## Decision

- Releasing the keys or the touch stick coasts: no velocity goal, the heading is kept. This holds with or without
  a lock (under a lock the lock keeps the heading as before).
- While keys are held the flight computer still flies the ship where it points (sideways drift cancelled); the
  drift toggle (X) still turns that off. A/D, W/S, Q/E and the stick are unchanged.
- To slow down the player thrusts against the motion (S on ships with reverse thrust, or turning and burning by
  hand).
- Later (Niklas): pilot assists such as auto-braking become configurable, through the behavior system for the
  player's own ship, together with key bindings. Not designed yet.

## Consequences

- `sim::flight` never uses `HeadingGoal::Auto` for player input any more; maneuvers of behaviors still use it.
- Every determinism stream with player input changes; the new hashes are in `docs/sim.md`.
- On touch, stopping means pushing the stick against the motion; there is no "let go to stop" any more.
