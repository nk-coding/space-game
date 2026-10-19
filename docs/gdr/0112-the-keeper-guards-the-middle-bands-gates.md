# GDR-0112: The Keeper guards the middle band's gates: a ring of four shielded cannon pods round a beam-laser heart, each on a core of its own

- Status: Accepted (provisional, lead), 18 Oct 2026, for Niklas to review
  2026-10-19: the inner band no longer uses `deep_gate` and the Keeper: its gates have the Spire ([GDR-0116](0116-the-spire-guards-the-inner-bands-gates.md)).
- Date: 2026-10-18
- Decided by: lead session (content batch of 18 Oct 2026, the Keeper; default for Niklas to review)
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md) (a guardian at every band gate), [GDR-0099](0099-the-grown-and-the-outer-reach.md) A7, [GDR-0103](0103-the-grown-in-play-defaults.md) (the bastion), [GDR-0031](0031-materials-and-fabrication.md) (gate shards), [GDR-0013](0013-vessels-and-piloting.md), [GDR-0100](0100-fight-balance-targets.md), `docs/results/picket-keeper-balance-2026-10-18.md`

## Context

The middle band's gates (trail `deep_gate`, also the inner band's) were a stand-in: the rim's Warden with its
raiders, easier than the outer reach's bastion gate before them. The band needs a guardian of its own that fights
differently from the first two (a heart between two fists) and is clearly harder than the bastion.

## Options considered

- **A bigger bastion** (more cannons, a third fist): the same fight again.
- **A ring of four shielded pods round a heart** (chosen): the Warden's and the bastion's breakable pieces with
  cores of their own, but four of them all round, each under its own bubble, so you open it pod by pod; the heart
  brings a new weapon to a guardian, the beam laser, shorter than the pods' cannons.
- **A station guardian** (anchored, GDR-0110): nothing to kite at a gate fight; kept for outposts.

## Decision

- The `automaton_keeper` (11×9, automatons, tags `boss`, `middle`, not `enemy`; lead default): four 3×3 pods at
  its corners, each joined to the heart by one bridge block, each with an automaton core, a shield emitter and a
  cannon (5 of its 6 MW); the front pods' cannons face forward, the rear pods' backward. The heart: three cores, a
  fusion reactor, two beam lasers between the front pods, two shield emitters and a torch drive. It flies the
  automatons' `raider_basic`. Shot off, a pod turns on its own core and fights on.
- The `keeper_gate` encounter: the Keeper and three hunters, wake radius 300 m. The `deep_gate` trail ends there
  and gives a gate shard, like every guardian's trail; its `stub` tag is gone. The inner band keeps the trail (and
  the Keeper) until it has a guardian of its own.
- Tuning (seeds 1–120, the voyage runner; Runner and one skiff / two skiffs): the gate alone clears 38 / 57 %
  against the bastion gate's 82 / 89 %; behind a raider pack and a hunter pack 10 / 20 % against 22 / 39 %.

## Consequences

- Ships tagged `middle` are left out of the scenarios that spawn every blueprint, as the outer reach's are.
- The middle band's clusters are still the uncharted stub (the rim's automatons); a fleet that reaches the Keeper
  should be stronger than a Runner and skiffs, so its numbers are to be re-measured when the band has its race.
