# GDR-0103: The Grown in play, lead defaults: three ship classes on a ganglion beside the heart, prows that charge from thorn range, small groves, a bastion at each outer gate, elders at the end of their own trail, one Grown research chain

- Status: Accepted (provisional, lead), 18 Oct 2026, for Niklas to review
- Date: 2026-10-18
- Decided by: lead session (defaults within [GDR-0099](0099-the-grown-and-the-outer-reach.md), Niklas's decisions)
- Related: [GDR-0099](0099-the-grown-and-the-outer-reach.md), [GDR-0102](0102-grown-mechanics-and-parts-defaults.md), [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0027](0027-races.md), [GDR-0031](0031-materials-and-fabrication.md), [GDR-0024](0024-tech-trees-no-mk-levels.md), [GDR-0025](0025-research-from-salvage.md), [GDR-0047](0047-ramming-crushes-inward-bumps-free.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0089](0089-play-test-15-oct-speed-salvage-waves-station.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0100](0100-fight-balance-targets.md), `docs/results/outer-balance-2026-10-18.md`

## Context

GDR-0099 decided what the outer reach brings (A1–A7); GDR-0102 built its mechanics and parts (phase 1). Phase 2 puts
the Grown in play: their ships and behaviors, the outer reach's sites and cluster types instead of the automaton
stand-ins, the Grown research family, a guardian at each outer gate and the giants as boss sites. This record holds
the lead's choices. All numbers are placeholders in content.

## Options considered

- **The Grown's mind**: a part of their own, the `grown_ganglion` (logic, power, a little healing; no programmable
  matter) (chosen); the automaton core the phase 1 test fixture used (not chosen: Grown salvage would give
  programmable matter, and a precursor drone's core in a living ship reads wrong).
- **How a rammer rams**: a new built-in behavior action `ram` (chosen: the flight computer brakes every approach, so
  `approach(0m)` arrives at a few m/s, under the ram threshold); approaching through the target with a negative
  range (not chosen: no such parameter, and it would still brake).
- **When it rams**: close in to thorn range facing the target, then charge from within 45 m in the last 3 s of a
  10 s cycle (chosen); circle and charge out of the orbit (not chosen: a ship with only stern thrust cannot take
  out the orbit's sideways speed, so the charge slid past or hit with its thrusters); charge at once (not chosen: the
  first charge crushed a Runner with an escort, a grove cleared about a third).
- **Where the prow sits**: out in front, the thorn guns a row back (chosen: the prow is what touches); the prow
  between two thorn guns in one row, as the phase 1 fixture (not chosen: a charge off the centre line hit with a
  thorn gun).
- **The research family**: one chain of three, regrowing tissue → prows and thorns → the heart (chosen: A4's tissue
  and prow first, the heart dearest; one row keeps the research screen within its height); four separate nodes (not
  chosen: four rows or columns did not fit the 1280 × 720 tree). The heart is researchable (it is the Grown's core
  tech); the ganglion is not (their own, like the automaton plate).
- **Rim stragglers near the border**: a cluster type rolled only within a distance of the band's outer edge (a
  band's `border` weight) (chosen); stragglers mixed into every outer cluster (not chosen: A5 places them near the
  border).
- **The outer gates' guardian**: a new precursor guardian, the `automaton_bastion` (the Warden's build with a
  twice-shielded, two-core heart and three cannons), with a gunner and a hunter (chosen); the Warden again (not
  chosen: the gates further in should be harder).
- **The elders**: the `grown_elder` (10 × 8, two hearts, two prows, eight thorn guns) with a rammer at an
  `elder_grove`, the end of a trail of its own (`elder_grove`, a salvage reward, two per galaxy) (chosen: A7, separate
  boss sites, found by following signals like the rim's caches); a cluster type of their own (not chosen: a boss
  would sit in plain sight).

## Decision

- **Ships** (faction `grown`, wary): `grown_rammer` (4 × 4: prow, two thorn guns, heart, ganglion, two thrusters),
  `grown_thornback` (4 × 4: four thorn guns, no prow, two ganglions), `grown_guardian` (6 × 6: prow, four thorn guns,
  a thick tissue hide, two ganglions), `grown_elder` (the boss). The heart sits right behind the prow, the ganglion
  beside the heart (a heart works only on the controlling piece, GDR-0102).
- **Behaviors**: `grown_rammer` (`ram` within 45 m for the last 3 s of every 10, else `approach(80%)`),
  `grown_thornback` (the raider's cycle at 75 % of thorn reach, aimed at weapons), `grown_guardian` (`ram` within
  45 m for the last 3 s of every 12, else `approach(70%)`). The `ram` action: top speed straight at the target,
  the nose on it, never braking (`Maneuver::Ram`); anyone may fly it.
- **Sites**: `grove_pair` (a rammer and a thornback), `grove` (a rammer, 0–1 thornbacks, 1–2 Grown wrecks as bait),
  `guarded_grove` (a guardian and a rammer, a wreck), `grown_dead` (wrecks only), `rim_stragglers` (1–2 raiders,
  0–1 gunners), `bastion_gate`, `elder_grove`.
- **Cluster types** of the outer reach (no longer a stub): `grove_reach` (weight 3), `scorched_deep` with groves
  beside its hot star (weight 2; its id kept so saves keep it), `border_drift` (weight 4, only within 0.45 S of the
  rim's border: stragglers and the first groves). Gates: `outer_gate` trails (3–4 signals) to a `bastion_gate`.
  Trails: 2 `elder_grove`. The middle, inner and core bands stay stubs.
- **Research** (`grown`, coral, last on the screen): `regrowing_tissue` (8 Grown data) → `living_weapons`, "Prows
  and thorns" (14 Grown, 4 kinetic: the ram prow and the thorn gun) → `grown_heart` (24 Grown, 2 programmable
  matter). Every Grown part's salvage and grown tissue's give Grown data.
- **Difficulty** (A6, the headless voyage runner, 120 seeds, test voyages built from the outer sites;
  `docs/results/outer-balance-2026-10-18.md`): a Runner with one cannon skiff clears a grove run 55 %, a guarded
  grove 74 %, a border drift 75 %, the bastion's gate 52 % and an elder grove 44 % (the Runner with the plain skiff
  at the gate 42 %, the Warden's 42 %); a second escort adds 5–10 points. Counts are the knob (GDR-0100).
- **Around it**: the voyage runner's pilot opens fire on a wary site it heads for (it never wakes on sight); the
  test range's waves leave out wary factions' ships (a wary race never comes at you in waves) and the bastion (not
  tagged `enemy`), so they stay as Niklas tuned them (GDR-0089); the determinism scenarios that spawn every
  blueprint leave out the outer reach's (tag `outer`). Saved designs carrying `automaton_cannon` are refitted with
  `reverse_engineered_cannon` when the game loads (the part's `replaces`), so designs from before 18 Oct keep
  working.

## Consequences

- A career made before keeps its galaxy (the save holds it): its outer clusters of the old stand-in type
  (`uncharted_deep`) keep the automatons and its outer gates the Warden; `scorched_deep` clusters get the Grown. A
  new game rolls the outer reach as above (the galaxy hash changes, `GENERATOR_VERSION` 3).
- The rim's voyages are unchanged run for run (no wary site, no new ship there).
- The base atlas grew past 8 × 8 units with the Grown's parts (tier 512 is 4096 × 5120); a GPU limited to 4096
  caps at tier 256.
- Open: the Grown's materials (GDR-0031), heat research and a heat shield, what the HUD shows of a heart, the
  bastion's and the elder's looks beyond reused parts, whether existing careers should re-roll their outer reach.
