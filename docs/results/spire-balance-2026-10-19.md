# The Spire: balance runs (19 Oct 2026)

For [GDR-0116](../gdr/0116-the-spire-guards-the-inner-bands-gates.md). Measured with the balance harness's voyage mode
(`../space-game-balance`, `--voyages --fleets`, the game's headless voyage runner, the content read at run time) as
in [the Keeper's runs](picket-keeper-balance-2026-10-18.md): a copy of the pack carried two test voyages built from
the gate sites (never in the game), radius 1200 m, the gate the only site and the goal:

| Test voyage | Site |
|---|---|
| t_keep | `keeper_gate` (the Keeper and three hunters) |
| t_spire | `spire_gate` (the Spire and its escort) |

Clear = goal cleared and home. Late fleets, all with the research variant
`research=shield_regen_3+research=thruster_speed_3+research=pulse_rate_3+research=radiator_vent_3` (every I–III
refinement chain, [refinement-2026-10-18](refinement-2026-10-18.md)); the skiff fits are the missile runs'
(`../space-game-balance/fits/`, [missiles-2026-10-19](missiles-2026-10-19.md)):

| Fleet | Ships |
|---|---|
| mixed | the Runner, a cannon skiff, a rocket skiff, a point-defence skiff |
| guns + rockets | the Runner, two cannon skiffs, a rocket skiff |
| lasers | the Runner, three rim skiffs |
| R+S | the Runner and one rim skiff (the Keeper's own measure) |

Load average about 1.6, native release build.

## Final numbers (as committed; seeds 1–240; clear / home / lost / out of time, clear %)

| Fleet | t_keep | t_spire |
|---|---|---|
| mixed | 195 / 36 / 7 / 2, **81 %** | 107 / 82 / 50 / 1, **45 %** |
| guns + rockets | 197 / 35 / 7 / 1, 82 % | 96 / 97 / 46 / 1, **40 %** |
| lasers | 158 / 65 / 14 / 3, 66 % | 35 / 132 / 70 / 3, **15 %** |
| R+S | 82 / 135 / 23 / 0, 34 % | 3 / 171 / 65 / 1, 1 % |

A strong late fleet with guns or missiles clears the Spire gate alone 40–45 % (the target 30–45 %), about half as
often as the Keeper gate (81–82 %); a laser fleet is meant to lose here (15 % against 66 %). The mixed fleet beats
guns and rockets because its point defence takes torpedoes and the lancers' rockets.

## Steps on the way (seeds 1–60, then 1–120; mixed / lasers clear %)

First layout: wards capacity 4, regen 0.4, the escort a lancer and two hunters.

| Variant | mixed | lasers |
|---|---|---|
| first layout (1–60) | 67 | 52 |
| wards 8 / 0.8 (1–60) | 65 | 43 |
| wards 12 / 1.0 (1–60) | 65 | 38 |
| wards 8 / 0.8, two lancers, two hunters (1–60) | 47 | 17 |
| wards 8 / 0.8, two lancers, three hunters (1–60) | 35 | 10 |
| wards 8 / 0.8, a lancer, two hunters, two gunners (1–60) | 30 | 13 |
| wards 12 / 1.0, two lancers, two hunters (1–120) | 44 | 18 |
| wards 8 / 0.8, two lancers, two hunters (1–120) | 46 | 24 |
| wards 8 / 0.8, two lancers, three hunters (1–120) | 34 | 12 |
| wards 4 / 0.4, two lancers, three hunters (1–120) | 38 | 17 |
| wards 8 / 0.8, two lancers, two hunters, the plain emitter's shares (energy 1, kinetic 0.5, missile 1) (1–120) | 40 | 22 |
| two lancers, two hunters, wards with no shares (no shield at all) (1–120) | 55 | 38 |
| wards 20 / 1.5, two lancers, two hunters (1–120, chosen) | 42 | 14 |
| wards 30 / 2.5, two lancers, two hunters (1–120) | 42 | 9 |

## Reading

- **The escort sets the level, the wards set who wins.** Ward capacity barely moves the gun fleets (42–46 % from
  capacity 8 to 30) but cuts the laser fleet from 24 to 9 %. Capacity 20, regen 1.5 was chosen: lasers get
  little through, without making the ward wholly immune (a laser fleet still clears one run in seven).
- **The shares matter for gun fleets:** the plain emitter's shares (missiles absorbed whole, half a shell) cost the
  mixed fleet 6 points against the prism ward's; no wards at all would lift it to 55 %.
- The runner fights the gate site alone here; behind packs (the Keeper's t_keep3) every gate is far harder.
  The inner band's clusters are still the uncharted stub; re-measure the gate in a run when the Lattice exist.

## Follow-up: the one-pod lancer (19 Oct 2026, later)

The automaton lancer was cut to one rocket pod the same day ([GDR-0115](../gdr/0115-missiles-and-point-defence.md)),
which made the Spire's escort lighter. Re-measured alone, the same harness, fleets and research variant, seeds 1–240
(the t_spire test voyage as above; clear / home / lost / out of time, clear %; load average about 2):

| Escort | mixed | guns + rockets | lasers |
|---|---|---|---|
| two lancers, two hunters (as committed, one-pod lancers) | 139 / 58 / 40 / 3, 58 % | 122 / 77 / 38 / 3, 51 % | 59 / 132 / 49 / 0, 25 % |
| **three lancers, two hunters (chosen)** | 102 / 96 / 40 / 2, **42 %** | 109 / 99 / 32 / 0, **45 %** | 29 / 153 / 56 / 2, **12 %** |
| two lancers, three hunters | 34 % | 36 % | 13 % |
| three lancers, three hunters | 31 % | 23 % | 6 % |
| two lancers, two hunters, a gunner | 24 % | 25 % | 5 % |

Three lancers put three rocket pods in the escort (four before the cut) and the strong late fleets back at
42–45 %, inside the 30–45 % target; the Runner and one skiff clears it 0 of 240. The mixed fleet no longer leads
guns and rockets (42 against 45 %, within the seeds' noise): one rocket pod fewer leaves its point defence less to do.

Screenshot (1280×720): `spire-fight-2026-10-19.png`, a fight code of the Runner and three rim skiffs against the Spire,
three lancers and two hunters (seed 2, 18 s in: the Spire's four prism wards over its spine). The fight viewer's title
wraps over the HUD's top left at this width when the sides list many designs (open).
