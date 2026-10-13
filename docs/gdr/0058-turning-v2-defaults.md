# GDR-0058: Turning v2 defaults: the numbers, RCS couples and when engines help turning

- Status: Accepted (provisional, lead)
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); numbers and details by the turning agent
- Related: [GDR-0055](0055-slower-crisper-turning.md) (the decision these implement), [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md), [GDR-0042](0042-agility-target.md), `docs/sim.md` "Flight" and "Turning and speed of the base ships"

## Context

GDR-0055 set the direction (keys ramp to about 1.5 rad/s, cores tiny, gyros modest, RCS strong, the Runner about
1.5 s per 90°, differential thrust as a last resort, the whole-speed fade). Implementing it needed numbers and a
few rules nobody had decided.

## Decision

Numbers (placeholders): logic core 50 kN·m (was 500), gyro 1250 kN·m (was 5000), RCS nozzle 200 kN (was 60),
automaton core 300 kN·m (was 600); `key_turn_rate` 1.5 rad/s; the key ramp starts at 20 % and reaches the cap in
0.8 s. Measured 90° turns: raider 0.57 s, Halvex Runner 1.50 s (with 2 RCS pods 1.17 s, with 4 pods 0.97 s),
sample early 1.60 s, sample mid 3.40 s. Acceleration is unchanged.

Rules:
- RCS pods turn the ship with **couples** (nozzles on opposite sides firing together, no net push), and couples
  ignore the speed fade, so pods turn at any speed.
- The keys (A/D) never open differential thrust; ships without gyros or RCS turn slowly on the keys (sample mid
  about 0.47 rad/s).
- Big automatic turns (to a heading or a target) open differential thrust and gimbals after 6 ticks (0.2 s) of
  missing torque, until the turn is done; never to stop a spin.
- The ramp lives in the client, which sends the shaped turn rate as the pilot input; the lock's A/D nudge keeps
  its GDR-0040 rate.

## Consequences

- Open for Niklas after flying it: RCS nozzle 200 kN also makes pods cheap low-speed boosters (Runner + 2 pods
  accelerates at 13.8 instead of 10.3 m/s²; 150 kN is the alternative); the raider at 0.57 s (alternatives 0.40 s
  or about 0.7 s); whether keys may use differential thrust after a delay; `assist_after` 6 ticks vs about 15 with
  stronger gyros; the lock nudge's lag (the nose trails the nudge by up to 0.29 rad).
- The sandbox fight balance has not been re-measured with the slower turning.
