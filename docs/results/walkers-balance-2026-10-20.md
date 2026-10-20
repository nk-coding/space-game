# The Phase walkers: blinks, lances and the phase colossus (20 Oct 2026, GDR-0127)

## Voyages

The game's headless voyage runner (`cargo run --release -p sim --example voyage`) with `--pack dir`: a small mod pack
in a scratch directory (never in the game) adds test voyages built from the content's encounters, radius 1000 m, sites
400–700 m apart: t_keep (keeper_gate alone, the middle band's gate, for scale), t_watch (lance_watch, lattice_dead,
lance_watch: the Lattice's, for scale), t_wpack (walker_pack, walker_dead, walker_pack), t_strike (walker_pack,
walker_strike) and t_colossus (walker_dead, colossus_site, the boss). Fleets as in `lattice-balance-2026-10-20.md`:
the Halvex Runner with a cannon skiff (R + C: the rim skiff with its pulse lasers swapped for reverse-engineered
cannons), with two plain rim skiffs (R + 2S) and with two cannon skiffs (R + 2C). Seeds 1–60 per cell, goal cleared
and home; load average about 5–6 (two other agents building). The reference rows match the Lattice's runs seed for
seed (t_keep 42 / 60 / 48 %, t_watch 48 / 45 / 50 %).

| Voyage | R + C | R + 2S | R + 2C |
|---|---|---|---|
| t_keep (the middle band's gate, for scale) | 42 % | 60 % | 48 % |
| t_watch (two Lattice lance watches, for scale) | 48 % | 45 % | 50 % |
| t_wpack (two walker packs) | 47 % | 60 % | 60 % |
| t_strike (a pack, then a strike) | 42 % | 53 % | 50 % |
| t_colossus (the phase colossus and two skirmishers) | 17 % | 30 % | 35 % |

Lost (of 60): t_wpack 4 / 9 / 4, t_strike 9 / 8 / 9, t_colossus 14 / 14 / 10; the rest came home without the goal.
So a walker cluster is about a Keeper's gate or a Lattice watch for a Keeper-capable fleet (pack 60 %, strike
50–53 %), and the colossus is the band's other boss, a little harder than the Lattice's resonator (25–33 %). The
outer-reach fleet (R + C) has no business there.

**The blink's share.** The same pack with walker drives that carry nothing (`blink.mass` 0.01: no walker ever
blinks), 60 seeds, R + C / R + 2S / R + 2C: t_wpack 75 / 78 / 88 %, t_strike 65 / 72 / 72 %, t_colossus 25 / 28 / 37 %.
Blinking costs the fleets a fifth to a quarter of their clears against packs and strikes; against the colossus it
changes little (not looked into further).

**The lance's share.** Lances that strike the first part only (`keep` and `armor` 0.001), 60 seeds: t_wpack 87 / 97 /
97 %, t_strike 73 / 87 / 92 %, t_colossus 57 / 75 / 78 %. Running through the hull is most of the walkers' threat: one
skirmisher's shot through the Runner struck six to nine parts in the traces (a pulse laser, a capacitor, a reactor, a
gyro and a radiator in one striker shot).

## How the numbers came about

The first cut (drive: 0.8 s charge, 7 s cooldown; lance: 1 damage/s in 0.8 pulses/s, so 1.25 a shot) cleared every
walker voyage (30 seeds, R + C / R + 2S / R + 2C): t_wpack 93 / 100 / 93 %, t_strike 83 / 80 / 87 %, t_colossus 47 /
70 / 73 %. A traced fight (a striker and two skirmishers against the Runner and two skiffs, 200 m) showed why: the
walkers blinked in at 2 s, behind the fleet, and died within reach 2–4 s later, before their drives had cooled; the
hit-and-run never ran. With a 4 s cooldown, a 0.6 s charge and the lance at 1.6 damage/s in 0.5 pulses/s (3.2 a shot:
fewer, harder strikes) the skirmishers blinked in, lanced, blinked out at about 7 s, came back at about 12 s and
again later; that is the cut above (kept).

## Arena, determinism and cost

`crates/core/arena/fights/walker_strike.ron` (a striker and two skirmishers against the Runner and two rim skiffs,
200 m; 253 build points against 97), 60 seeds: side 0 won 55, side 1 won 1, undecided 4, mean 31 s; the walkers dealt
about as much damage as they took (seed 1: 39 / 43). 312 fights/s on 9 threads, 0.29 M ticks/s (`runner_vs_raiders`
in the balance check: 0.22 M ticks/s): a blink is one shape query per landing spot tried, a lance one grid walk.
`scripts/determinism-check.sh --fight walker_strike`: native against wasm32-wasip1 **identical**, seed 42 (675 ticks,
`2114907c22a41493`) and seed 7 (2700 ticks, `6a433a8dfbc53702`, a fight to the time limit; the walkers dealt 83 damage).

The balance check (`scripts/arena-check.sh`): all seven fights the same, fight for fight; the rules fingerprint moved
with the inner band's content (both races), baseline refreshed.

The galaxy: over seeds 1–200 the inner band has 1767 clusters, 504 of them phase rifts (28 %, weight 2 beside the
Lattice's 5), and the phase colossus trail lies in 147 galaxies (74 %); in the others no phase rift was free for its
end (the trail's `ends_in`), so the colossus is left out there.

## Reproduce

```sh
cargo build --release -p sim --example voyage
# a pack dir with pack.ron = (id: "walkers_test", ..., depends: ["base"]) and voyages/zz_walkers.ron holding the test
# voyages above (band "inner", radius 1000, spacing (400, 700)); skiff_cannon.ron: rim_skiff.ron with
# "reverse_engineered_cannon" for "pulse_laser"
target/release/examples/voyage --pack <dir> --voyage t_wpack --design halvex_runner \
  --escort rim_skiff,rim_skiff --seeds 1..60
cargo run --release -p arena-cli -- fight crates/core/arena/fights/walker_strike.ron --seeds 60
scripts/determinism-check.sh --fight walker_strike --seed 42
```
