# The Lattice: prism relays, refractive crystal and the resonator (20 Oct 2026, GDR-0126)

## Voyages

The game's headless voyage runner (`cargo run --release -p sim --example voyage`), with the new `--pack dir` option: a
small mod pack in a scratch directory (never in the game) adds test voyages built from the content's encounters,
radius 1000 m, sites 400–700 m apart: t_keep (keeper_gate alone, the middle band's gate), t_spire (spire_gate alone,
the inner band's gate), t_carrier (swarm_patrol, swarm_dead, hive_carrier: a middle-band voyage for scale), t_watch
(lance_watch, lattice_dead, lance_watch), t_pair (lance_watch, lance_pair), t_site (lattice_dead, crystal_site) and
t_res (lattice_dead, resonator_site, the boss). Fleets as in `swarm-balance-2026-10-19.md`: the Halvex Runner with a
cannon skiff (R + C: the rim skiff with its two pulse lasers swapped for reverse-engineered cannons), with two plain
rim skiffs (R + 2S, lasers) and with two cannon skiffs (R + 2C). Seeds 1–60 per cell, goal cleared and home; load
average about 7 (two other agents building).

| Voyage | R + C | R + 2S | R + 2C |
|---|---|---|---|
| t_keep (the middle band's gate, for scale) | 42 % | 60 % | 48 % |
| t_carrier (middle band, for scale) | 27 % | 38 % | 33 % |
| t_spire (the inner band's gate) | 2 % | 3 % | 20 % |
| t_watch (two lance watches) | 48 % | 45 % | 50 % |
| t_pair (a watch, then a lance pair) | 20 % | 37 % | 37 % |
| t_site (a crystal site) | 75 % | 72 % | 77 % |
| t_res (the resonator and two needles) | 10 % | 25 % | 33 % |

So a Lattice cluster is about a Keeper's gate for a Keeper-capable fleet (watch 45–50 %, pair 37 %), a crystal site
is the easier stop (72–77 %), and the resonator is the band's boss: 25–33 % for the Keeper-capable fleets, between the
middle band's carrier and the Spire. The outer-reach fleet (R + C) has no business there (10–48 %).

**Refraction's share.** The same pack with `refract: 0` (crystal that turns nothing), 60 seeds, R + 2S / R + 2C:
t_watch 70 / 67 % (45 / 50 with refraction), t_pair 45 / 53 % (37 / 37), t_site 97 / 97 % (72 / 77). The crystal
costs the fleets a quarter to a third of their clears; the cannon fleet too, since the Runner fires two beam lasers
and two pulse lasers.

## How the numbers came about

The first cut (focus emitter 1.3/s at 50 m, a needle of three prisms: 88 m and 1.9/s per needle, watches of two or
three) cleared no voyage: t_watch 0 % for R + 2S and R + 2C (30 seeds), the Runner disarmed or lost within a minute
and without a kill. The runner's pilot closes on its target from out of reach; a needle that outreaches every gun the
fleet has shoots its nose guns off on the way in. What was tried (t_watch, R + 2C, 30 seeds unless noted):

| Change | t_watch |
|---|---|
| first cut (50 m, 1.3/s, three prisms, 2–3 needles a watch) | 0 % |
| refract 0 | 0 % (R + 2S 0 %) |
| 1.0/s | 3 % |
| 1.0/s, watches of two | 3 % |
| 0.9/s, prisms +20 % reach and +10 % damage, watches of two | 13 % |
| the same without `kite` in the needles' behavior | 0 % |
| hit points of crystal and cores a third | 3 % |
| 35 m (61 m relayed), 1.3/s, watches of two | 43 % |
| 0.5/s, watches of two | 23 % |
| 40 m (70 m relayed), 1.0/s, watches of two | 7 % (R + 2S 7 %) |
| 36 m, 1.2/s (63 m relayed), watches of 2–3 | 10 % (R + 2S 17 %) |
| 36 m, 1.2/s, the needle with two prisms (54 m, 1.56/s), watches of 2–3 (kept) | 47 % (R + 2S 43 %) |
| the same, watches of two | 77 % |

So reach decides it: a needle that outreaches the starters' pulse lasers (60 m) and the cannon (75 m) by much wins
whatever its damage. Kept: the focus emitter 1.2/s at 36 m (alone it is a short gun: the Lattice never fire it
without prisms), the needle with two prisms (54 m, 1.56/s), the lance with three per spine (63 m, 1.73/s each), the
resonator's spokes four each (72 m, 1.92/s each, three into the mouth). The prism's own shares stay +25 % reach and
+15 % damage, so a player's pulse laser behind four prisms reaches 120 m at 1.6/s.

The resonator alone (36 m emitters) cleared 80–87 % (30 seeds): the pilot comes in from whatever side it meets it on
and the station turns slowly. With two needles at its side 37–40 %, with a guard instead 53–60 %; the site keeps two
needles (`resonator_site`).

## Arena and cost

`crates/core/arena/fights/lattice_lances.ron` (a lance, a guard and a needle against the Runner and two rim skiffs,
200 m), 60 seeds, one thread: side 0 won 19, side 1 won 20, 21 undecided at 90 s; 23 fights/s, 0.05 M ticks/s
against `runner_vs_raiders`' 52 fights/s, 0.09 M ticks/s (four ships against six; a relayed beam is one ray cast like
any beam, a glance one more).

Determinism, native against wasm32-wasip1 (`scripts/determinism-check.sh`): `--fight lattice_lances` seed 42 (2700
ticks, `702da3ede43c5b5f`) and seed 7 (1214 ticks, the fight's end, `8ddf91f65c6c190d`), `--scenario grown` seed 42
(1800 ticks, the needle's site warns, wakes and fights, `10155c752bb919f9`): **identical**.

## Reproduce

```sh
cargo build --release -p sim --example voyage
# a pack dir with pack.ron = (id: "lattice_test", ..., depends: ["base"]) and voyages/zz_inner.ron holding the test
# voyages above (band "inner", radius 1000, spacing (400, 700))
target/release/examples/voyage --pack <dir> --voyage t_watch --design halvex_runner \
  --escort <skiff_cannon.ron>,<skiff_cannon.ron> --seeds 1..60
```
