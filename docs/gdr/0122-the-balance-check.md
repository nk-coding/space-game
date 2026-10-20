# GDR-0122: The balance check: seven fixed fights against a stored baseline, flagged past 15 points of wins, 0.12 score or 15 % time

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review
- Date: 2026-10-20
- Decided by: lead (balance check agent; Niklas was away, defaults where GDR-0032 and `docs/plans/coop-arena.md` A3
  leave things open)
- Related: [GDR-0032](0032-balancing-arena-evolution.md) ("a short run checks every data change"),
  [GDR-0100](0100-fight-balance-targets.md), [ADR-0036](../adr/0036-arena-crate.md), [ADR-0008](../adr/0008-local-git-only.md),
  `docs/plans/coop-arena.md` 3.9 and 6 (track A3), `docs/arena.md` "The balance check"

## Context

GDR-0032 settled that a short arena run checks every data change; coop-arena A3 sketched it as
`scripts/arena-check.sh`, a fixed set (placeholder: about 50 fights, under 10 s) against a stored baseline. Open were
the fights, the seeds, what is compared and how far a number may move. The fights are deterministic, so unchanged
content repeats bit for bit; but any data change re-rolls every fight (the sim is chaotic), so the check compares
samples and its tolerances must sit above that noise.

## Options considered

- **Exact hashes**: fails on every change, so it says nothing about balance (the determinism check already does it).
- **Few fights (about 50) with tight tolerances**: a re-roll alone would flag (the sample spread of 50 fights is
  ±0.1 in win rate).
- **Enough seeds per fight that a re-roll stays about 3 σ inside wide-ish tolerances** (chosen): on this VM's 4 cores
  1696 fights still take about 6 s.

## Decision

- **The set** (`crates/core/arena/check/set.ron`): the four fights of `crates/core/arena/fights/` plus three new 1v1
  or 1v2 specs for the factions they miss; each runs its spec's seed (1) and the ones after it:

  | Fight | Seeds | What it covers |
  |---|---|---|
  | `runner_vs_raiders` | 300 | the standard measurement fight |
  | `skiff_mirror` | 100 | side bias (should stay at 0.50) |
  | `skiff_vs_lancer` | 400 | the starter against an early automaton (close, so noisy: more seeds) |
  | `mid_vs_warden` | 200 | the rim guardian: shields, cannons, chain reactions |
  | `runner_vs_rammers` | 300 | the Grown: rams and regrowth |
  | `runner_gauntlet` | 300 | waves (the test range's and the lab's gauntlets) |
  | `hive_flood` | 96 | the Swarm: drones (the dearest fight, 24 drones) |

- **Compared per fight**: each side's win rate, side 0's mean arena score and the mean fight length, against
  `crates/core/arena/check/baseline.ron` (RON, written by `--update`, committed). A fold of every fight's final hash
  is shown as `same` / `changed` but never fails.
- **Tolerances** (*placeholders*): win rates ±0.15 (absolute), score ±0.12, time ±15 % of the baseline's. Measured
  20 Oct 2026 over 1000 seeds per fight in blocks: at these seed counts the difference of two re-rolls has a standard
  deviation of at most 0.045 in win rate, 0.042 in score and 5.5 % in time, so the tolerances are 2.7–4 σ; five
  re-rolls (seeds from 1001, 2001, … 5001) all passed. A beam laser 30 % stronger fails two fights
  (`mid_vs_warden` score +0.17, `runner_vs_rammers` +0.13); a pulse laser 30 % stronger passes (it moved the hive
  fight's score by 0.08 only).
- **When**: after every content or sim change, locally (ADR-0008: no CI); the exit code is 1 when a fight leaves its
  tolerance. An intended shift is accepted with `scripts/arena-check.sh --update`, and the new baseline is
  committed with the change. A set file may override the tolerance per fight.

## Consequences

- A data change that moves a fight's balance by more than the tolerance shows up in about 6 s (plus the release
  build); smaller shifts pass unnoticed, as intended: the check flags large shifts, the lab and `arena matrix`
  measure small ones.
- Exploit hunts (A7) add their fights to the set; a fight added needs `--update` once.
- The baseline is a balance record in git: its diff shows which fights an accepted change moved.
