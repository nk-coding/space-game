# GDR-0100: Fight balance targets: each slice voyage harder than the last, a Runner and an escort clear the Warden about 40 %, shields collapse under sustained fire, encounter counts tuned first

- Status: Accepted (Niklas, 17 Oct 2026).
  2026-10-17: measured (`docs/results/slice-balance-2026-10-17.md`): B1 met by two counts (GDR-0073's status: the
  Warden with a Runner and an escort 44 %); B2 not met at capacity 3, regen 0.2: a bubble collapses in 0–9 % of the
  test range's waves (capacity 2 gives 16.5 %, 1.5 gives 31 % under a passive pilot); open for Niklas.
  2026-10-17 (second pass, `docs/results/slice-balance-2026-10-17b.md`): Niklas set B2's capacity to 2 (regen 0.2
  stays; [GDR-0021](0021-shields.md)): a bubble collapses in 16 % of the Runner's test-range waves under a passive
  pilot, about one in six. A different first enemy ([GDR-0101](0101-the-automaton-gunner.md), the automaton gunner)
  brings the starter to 57 / 37 / 22 / 15 % and the Runner to 74 / 65 / 40 / 25 %; B1 met (each voyage harder than
  the last for every fleet, a Runner with an escort 42 % at the Warden, unchanged); Corran Reach gained a gunner pair
  (a count, B3).
  2026-10-18 (`docs/results/slice-balance-2026-10-18.md`): Niklas had the player's cannon toned down. Research
  `automaton_cannon` now unlocks the player's own copy, `reverse_engineered_cannon` (rate and damage 1.2 → 1.1, a
  shot still 1 hit point); the automatons keep `automaton_cannon`, so no enemy fight changed. The cannon starter
  clears 91 / 63 / 38 / 28 % (was 93 / 68 / 46 / 37), below a Runner with an escort at the Warden (42 %); no count
  re-tuned.
  2026-10-17 (`docs/results/scenery-2026-10-17.md`, 120 seeds): asteroid rock in the slice's first three voyages ([GDR-0104](0104-scenery-rocks-filler-landmarks.md)) keeps the curve (each voyage at most as easy as the one before; the Warden's gate has no rock and is unchanged); clear rates fall up to 13 points (the Runner at Halvex Drift and Corran Reach), the cannon starter 92 / 65 / 38 / 26 → 82 / 65 / 34 / 26 %.
- Date: 2026-10-17
- Decided by: Niklas (interactively, 17 Oct 2026; the recommended option on every question)
- Related: [GDR-0073](0073-slice-tuning-v1.md), [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), [GDR-0061](0061-fight-balance-after-turning-v2.md), [GDR-0064](0064-lock-feed-forward-and-ai-flight-fixes.md), [GDR-0089](0089-play-test-15-oct-speed-salvage-waves-station.md), [GDR-0021](0021-shields.md), [GDR-0055](0055-slower-crisper-turning.md), [GDR-0058](0058-turning-v2-defaults.md), [GDR-0066](0066-phase-7-wave-1-defaults.md), [GDR-0099](0099-the-grown-and-the-outer-reach.md), `docs/results/slice-balance-2026-10-17.md`

## Context

The slice's balance was last measured on 13 Oct (GDR-0073, after GDR-0061's content and GDR-0064's flight fixes).
Since then several changes moved every fight: the speed governor and settling salvage (GDR-0089, whose numbers say
the balance results predate it), chain reactions (GDR-0084, GDR-0085) and the 16 Oct shield fix that stops
cannon shells slipping through a closing ship's bubble (GDR-0021's notes). Niklas set the targets the re-measurement
tunes to. The re-measurement is recorded in `docs/results/slice-balance-2026-10-17.md`.

## Options considered

Niklas chose the recommended option on each of the five questions. The other options offered are not recorded.

## Decision

- **The curve** (B1): GDR-0073's curve stays the target. Each slice voyage is harder than the one before, and a
  Runner with an escort clears the Warden about 40 % of the time (GDR-0073 measured 42 %). Re-measure and tune to it.
- **Shields** (B2): they collapse under sustained fire (GDR-0061). Capacity 3 and regen 0.2 stay, which collapsed a
  bubble in about one wave in five. Re-check this after the 16 Oct shell fix (GDR-0021).
- **The first tuning knob** (B3): encounter counts (pack sizes, escorts in content), as in GDR-0073. Other knobs (ship
  designs, weapon numbers, research costs) come after them.
- **Turn rates stay** (B4): the starter turns 90° in about 0.67 s (GDR-0066), the Halvex Runner in 1.5 s (GDR-0055,
  GDR-0058). They are not a balance knob.
- **Test-range waves stay** (B5): a wave's budget is half the flown design's build points, spent on the biggest
  enemies first (GDR-0074, GDR-0089).

## Consequences

- The tuning pass's numbers and before → after changes go in the results report.
- The shield re-check after the shell fix is part of the re-measurement.
- The same way of measuring (the headless voyage runner, the same seeds) sets the outer reach's target later
  (GDR-0099: a Runner-class flagship with 1–2 escorts).
- This closes the README's old open item on fight length; GDR-0061 and GDR-0064 had already resolved it.
