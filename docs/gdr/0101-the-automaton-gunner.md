# GDR-0101: The first voyages' enemy is the automaton gunner: twin cannons, aimed at the engines

- Status: Accepted (Niklas, 17 Oct 2026: a different first enemy, content only, the starter stays); the gunner's
  layout, behavior and placement are lead defaults (17 Oct 2026)
- Date: 2026-10-17
- Decided by: Niklas (the enemy); lead session (its numbers, for Niklas to review)
- Related: [GDR-0100](0100-fight-balance-targets.md), [GDR-0073](0073-slice-tuning-v1.md), [GDR-0021](0021-shields.md), [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), `docs/results/slice-balance-2026-10-17b.md`

## Context

Since the starter's shield emitters moved forward over its pulse lasers (16 Oct 2026), raiders hardly hurt it: it
cleared the first two voyages 96 % and 62 % of the time against GDR-0073's 58 % and 37 %. Encounter counts could
not close the gap, because more raiders cost the Halvex Runner (whose nose beams go first) three to six times
what they cost the starter (`docs/results/slice-balance-2026-10-17.md`). Niklas chose a different first enemy, one
that answers shields, made in content from parts that exist; the starter stays as it is.

## Options considered

- **A kinetic raider variant in content** (chosen by Niklas): a new ship and behavior from existing parts.
- Changing the starter's layout or the raider's aim at a shielded gun, or part numbers: not chosen (Niklas kept
  the starter; GDR-0100 B3 tunes counts first).

Among variants the lead measured (seeds 1–60 or 1–120, every raider of the first two voyages replaced): longer
attacks (4–7 s of every 9 instead of 3) hurt the Runner more than the starter; aiming at weapons too; aiming at
power or at the centre with two cannons cost both alike; two cannons aimed at the engines cost the starter about
half its first voyages and the Runner nothing (numbers in the report).

## Decision

- **`automaton_gunner`** (`ships/automaton_gunner.ron`, tag `enemy`, faction `automatons`): a 4×3 raider frame
  with two `automaton_cannon`s side by side in its nose, one `automaton_core` (its 6 MW run both cannons and one
  thruster, as in the hunter), plate armour; 21.2 build points (a raider 15.8, a hunter 23.6). Kinetic shells
  lose half to a bubble (GDR-0021), so two of them wear a shield down twice as fast as one.
- **`gunner_pack`** (`behaviors/gunner_pack.ron`, the gunner's own `behavior`): the rules of `raider_basic` (attack
  for 3 s of every 9, circle at 58 % of the reach for the other 6) with the target `nearest(engines)`.
- **Encounters**: `gunner_pair` (2 gunners), `raiding_party` (a gunner and a raider), `gunner_den` (3 gunners, wakes
  at 300 m).
- **Where**: the Shipyard approach is gunner pair ×2, derelict field, goal raiding party (was raider pair ×2, the
  field, goal raider pair); Halvex Drift gunner pair, precursor wrecks, core graveyard, goal gunner den (was a
  raider pair and the raider den); Corran Reach gains a gunner pair (its third voyage had become easier than the
  second for the starter); a wreck drift (`cluster_types/rim.ron`) holds 0–1 gunner pairs. The test range's waves
  draw every enemy ship (GDR-0074, GDR-0089), the gunner too; with its build points the starter's and the
  Runner's waves stay as they were.

## Consequences

- Clear rates over 240 seeds (`docs/results/slice-balance-2026-10-17b.md`, with shield capacity 2): the starter 57 /
  37 / 22 / 15 % (GDR-0073: 58 / 37 / 15 / 3), the Runner 74 / 65 / 40 / 25 % (60 / 67 / 45 / 20), a Runner with the
  starter as escort 82 / 70 / 51 / 42 %. Every fleet finds each voyage harder than the one before.
- The starter is lost more often on the first two voyages (17 and 48 runs of 240, were 2 and 5).
- The cannon starter is hardly touched by gunners (93 / 68 / 46 / 37 %): it clears the Warden about as often as a
  Runner with an escort (42 %). Open for Niklas.
- The raider pair and den now wait only in the kinds of cluster; the slice chain's test counts them there.
- Lab runs and fight codes made before are stale (the content's fingerprint changed); new enemies show in the
  lab's ARMS RACE (three enemy starts).
