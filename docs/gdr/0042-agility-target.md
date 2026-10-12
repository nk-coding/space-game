# GDR-0042: Agility target "Agile": the Halvex Runner turns 90° in ~0.8 s and reaches ~22 m/s in ~2.5 s; small ships are snappier

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (the content numbers and the flight-tuning changes that reach the target are the implementing
  agent's placeholders)
- Related: [GDR-0005](0005-fully-physical-ships.md), [GDR-0010](0010-turning-sources.md), [GDR-0041](0041-engine-rated-speed.md), `docs/sim.md` "Turning and speed of the base ships", `content/base/parts/propulsion.ron`

## Context

Niklas played the web build on 12 Oct 2026: the Halvex Runner felt really sluggish (90° in 3.3 s, 0.65 m/s²,
top speed about 10 m/s), and he wants more manoeuvrable ships in general. The default view is about 70 m wide.

## Options considered

- **Agile** (chosen): Halvex Runner 90° in about 0.8 s, 0 → top speed in about 2.5 s, top speed about 22 m/s
  (crosses the view in about 3 s).
- **Arcade snappy**: 90° in 0.5 s, 0 → top in 1.5 s, 30 m/s. Reason not recorded.
- **Weighty**: 90° in 1.3 s, 0 → top in 4 s, 16 m/s. Reason not recorded.

## Decision

- The Halvex Runner meets the "Agile" numbers; small ships (the automaton raider) are snappier than the frigate;
  the other ships (sample early, sample mid) follow from the same part numbers.
- Reached with **content numbers** (placeholders): thrust about 16× (thruster 200 kN, torch drive 700 kN, RCS
  nozzle 60 kN), gyro torque 5000 kN·m, logic core 500, automaton core 600, and the rated speeds of GDR-0041.
  `thrust_scale` and `torque_scale` stay at 1 as test sliders.
- Flight tuning (agent's change): `max_turn_rate` 3 → 6 rad/s and `turn_gain` 8 → 12 /s. With 3 rad/s and 8 /s no
  ship could turn 90° faster than 0.78 s however much torque it had, so the raider could not be snappier than the
  Runner.

Measured (90° from rest to within 1°; 0 → 95% of the top speed with the stick along the nose):

| Ship | Mass | 90° turn | Max α | Accel | Top speed | 0 → 95% top |
|---|---|---|---|---|---|---|
| Automaton raider | 15.8 t | 0.40 s | 35.5 rad/s² | 12.7 m/s² | 20 m/s | 1.70 s |
| Halvex Runner | 106.5 t | 0.77 s | 9.9 rad/s² | 10.3 m/s² | 22 m/s | 2.53 s |
| Sample early | 43 t | 0.90 s | 7.0 rad/s² | 9.3 m/s² | 20 m/s | 2.30 s |
| Sample mid | 88 t | 2.03 s | 1.5 rad/s² | 12.5 m/s² | 22 m/s | 2.10 s |

## Consequences

- Ships cross the view in about 3 s. The raider pack behavior was retuned (`raider_basic`: stand off at 85%,
  orbit at 67% of its reach) so a fight is still a fight (`docs/sim.md` "Behaviors").
- Ramming impulses roughly double: a Runner at its top speed into a raider at rest is about 380 kN·s (7 hp to
  each side under the v1 rule), head-on with a raider at its top speed about 720 kN·s (14 hp), two Runners
  head-on about 2950 kN·s (59 hp); raiders converging on a target bump each other at 20–30 m/s. The ramming rule
  is still open with Niklas.
- The sample mid has no gyros and still turns slowly (about 2 s): turning stays a drydock trade-off (GDR-0010).
- Turning with differential thrust now pushes the ship noticeably, and keyboard rotation of the Runner runs at
  about 4.6 rad/s. Niklas tunes further with the T panel.
