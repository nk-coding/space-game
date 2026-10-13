# GDR-0055: Turning is slower and crisper; RCS pods are the strong turners; top speed is capped by the whole speed

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively, after flying the new speeds)
- Supersedes: the turning target of [GDR-0042](0042-agility-target.md) (the acceleration target stays); the per-axis fade of [GDR-0041](0041-engine-rated-speed.md) for thrusters pushing with the motion; `docs/sim.md` design questions 14 and 15
- Related: [GDR-0010](0010-turning-sources.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md), [GDR-0049](0049-engine-plume-v1.md), `docs/sim.md` "Flight"

## Context

After the 12 Oct speed-up (GDR-0041, GDR-0042) Niklas likes the fast acceleration, but turning is far too
strong: the Halvex Runner turns 90° in 0.77 s on two gyros and two logic cores, A/D spin it at about 4.6 rad/s
(a 0.1 s tap turns it ~26°), so aiming with the keys feels slippery, and RCS pods hardly matter. Differential
thrust from off-axis engines adds sideways push nobody asked for. Separately, the per-thruster speed fade lets a
ship moving diagonally beat each rating (22 m/s forward plus 14 m/s sideways RCS ≈ 26 m/s).

## Options considered

- Slippery aim: **keys spin too fast for fine aim** (chosen), the nose keeps turning after release, or both.
- Runner 90° turn: **about 1.5 s** (chosen), about 1.2 s, about 2 s or slower.
- Sources: **cores tiny, gyros modest, RCS pods strong** (chosen), RCS main with gyros as a weak backup, or the
  same ratios scaled down.
- Differential thrust: **last resort only** (chosen), used freely, or never for turning.
- Diagonal top speed: accept it (the lead's recommendation), or **cap by the whole speed** (chosen).
- Plumes (`docs/sim.md` questions 17, 18): **no push** and **a faint full-reach streak only while a plume burns
  something** (both chosen, as built).

## Decision

- **Keyboard rotation** is capped at about 1.5 rad/s and ramps up while A/D are held: a tap nudges the nose a
  little, holding turns faster up to that cap (never above what the ship can do). Placeholder numbers.
- **Turning sources**: a logic core's built-in assist is tiny (it keeps a ship controllable, nothing more); each
  gyro helps moderately; RCS pods are the strong turners (they also strafe and brake). With its two gyros and two
  cores and no RCS, the Halvex Runner turns 90° in about **1.5 s**; RCS pods make a visible difference. Small
  ships stay snappier than big ones. Content numbers, placeholders.
- **Differential thrust** (off-axis engines and gimbals used for turning) is a last resort: the flight computer
  uses it only after gyros, cores and RCS have been at their limit for a few ticks.
- **Top speed**: a thruster that pushes with the ship's motion fades by the ship's whole speed (not only the
  speed along its own axis), so the slowest thruster pushing that way sets the cap; braking and turning keep full
  thrust as before.
- **Acceleration** stays as it is (GDR-0042: 0 → top speed in about 2.5 s for the Runner).
- **Plumes** don't push, and show their full reach faintly only while they burn something (GDR-0049's defaults
  for these two points, confirmed).

## Consequences

- RCS pods become a real drydock choice for agile ships; the Runner itself has none.
- The agility tests, the design report's turn estimate and the per-direction top speeds follow the new rules;
  state hashes change once (native and wasm must still agree).
- The plume numbers themselves (GDR-0049, `docs/sim.md` question 16) are still open.
