# GDR-0116: The Spire guards the inner band's gates: a long three-section spine with torpedoes and rockets under crystal wards that turn energy fire

- Status: Accepted (provisional, lead), 19 Oct 2026, for Niklas to review
- Date: 2026-10-19
- Decided by: lead session (agent WARDEN-INNER; default for Niklas to review)
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md) (a guardian at every band gate), [GDR-0027](0027-races.md) (the inner band is the Lattice's: refractive crystal armour), [GDR-0099](0099-the-grown-and-the-outer-reach.md) A7, [GDR-0112](0112-the-keeper-guards-the-middle-bands-gates.md) (the Keeper), [GDR-0021](0021-shields.md), [GDR-0115](0115-missiles-and-point-defence.md), [GDR-0031](0031-materials-and-fabrication.md) (gate shards), [GDR-0100](0100-fight-balance-targets.md), `docs/results/spire-balance-2026-10-19.md`

## Context

The inner band's gates used the middle band's trail (`deep_gate`) and its Keeper (GDR-0112) as a stand-in. The band
needs its own guardian, a clear step above the Keeper, that fights differently from the Warden and the bastion (a
heart between two fists) and the Keeper (a ring of shielded pods). Its band is the Lattice's (GDR-0027: refractive
crystal), so the guardian can carry the band's idea before the race exists. It should reuse existing rules.

## Options considered

- **A long spine whose sections break off** alone: the Keeper's pods again, only in a line.
- **Crystal wards that turn energy fire** (chosen, with the spine as its shape): a shield emitter whose absorb
  shares (GDR-0021's per-family table) take lasers whole and let shells and missiles through, so the fleet's
  weapon mix decides the fight; data only (a new part, no new rule).
- **Healing between phases:** needs a phase rule in the sim.

## Decision

- **The prism ward** (`prism_ward`, enemy only, locked, no research unlocks it yet): a 1×1 shield emitter of Lattice
  crystal, radius 2.6, capacity 20, regen 1.5, absorb energy 1.0, kinetic 0.2, missiles 0.25 (the plain emitter:
  capacity 2, regen 0.2, energy 1.0, kinetic 0.5, missiles 1.0). Energy fire barely wears it down; a cannon's shell
  or a warhead mostly passes.
- **The Spire** (`automaton_spire`, 8×14, automatons, tags `boss`, `inner`): three sections in a line, each on two
  automaton cores of its own and joined by one bridge block. The head: two torpedo launchers (120 m, further than
  any of the player's guns) and a ward; the body: a fusion reactor, two wards and two forward rocket pods on its
  flanks; the tail: the torch drive, a fission reactor, a ward and two rocket pods covering its back. Shot apart,
  each section fights on; only the tail can still fly. It flies the automatons' `raider_basic`.
- **The `spire_gate` encounter:** the Spire, two lancers and two hunters, wake radius 300 m.
- **The `inner_gate` trail** (3–4 signals, like the others) ends there and gives **two** gate shards (the last
  guardian before the core); the inner band's gates use it instead of `deep_gate`, which stays the middle band's.
- Ships tagged `inner` join the scenarios' deep tags (left out of the scenarios that spawn every blueprint).
- Tuning (seeds 1–240, the voyage runner, the gate alone, late research: shield regen, radiator vent, thruster
  speed and pulse rate 3): a mixed late fleet (the Runner, a cannon, a rocket and a point-defence skiff) clears 45 %
  against the Keeper's 81 %; guns and rockets (two cannon skiffs, a rocket skiff) 40 % against 82 %; the Runner and
  three laser skiffs 15 % against 66 %.

## Consequences

- An all-laser fleet is meant to lose here: the player is told by the ship's and the ward's texts and the wiki.
- Point defence earns a place (torpedoes, eight rockets with the lancers): the mixed fleet beats guns and rockets.
- The inner band's clusters are still the uncharted stub; re-measure when the Lattice exist. The prism ward is a
  candidate for the Lattice's research family (refractive armour), not unlocked by anything yet.
