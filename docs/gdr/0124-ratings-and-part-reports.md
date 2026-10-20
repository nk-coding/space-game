# GDR-0124: Ratings and part reports: Bradley–Terry on the Elo scale over a round robin, pick rate among the top half, three flags

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review
- Date: 2026-10-20
- Decided by: lead (ratings agent; Niklas was away, defaults where GDR-0032 and `docs/plans/coop-arena.md` A4 leave
  things open)
- Related: [GDR-0032](0032-balancing-arena-evolution.md) ("Elo or TrueSkill ratings; reports on pick rates, win rates
  and unused parts"), [GDR-0122](0122-the-balance-check.md) (the balance check), [ADR-0036](../adr/0036-arena-crate.md),
  `docs/plans/coop-arena.md` 2, 3.7, 3.9 and 6 (track A4), `docs/arena.md` "Ratings and part reports",
  `docs/results/arena-report-2026-10-20.md`

## Context

GDR-0032 settled that the arena rates designs (Elo or TrueSkill) and reports pick rates, win rates and unused parts;
the concept board's balancing section shows an example report with pick rate, win rate when used and the flags
DOMINANT, UNDERUSED and NEVER PICKED. Open were the rating method, what "top designs" and "when used" mean, where the
flags start, and what the report is computed from.

## Options considered

- **Elo** (online updates, one fight at a time): its result depends on the order of the fights and a K factor, and
  our fights come as a complete batch from fixed seeds, where no order is natural.
- **TrueSkill**: built for teams and uncertainty; heavier, and the arena's ratings are 1v1 for now.
- **Bradley–Terry, fitted to the whole batch** (chosen): the maximum-likelihood version of the model behind Elo's
  expected-score curve, so the numbers read like Elo, but independent of fight order and with no K to tune;
  Hunter's MM iteration converges in milliseconds for a pool of tens of designs. TrueSkill stays the option for team
  fights later (coop-arena 3.7).

## Decision

- **What is rated**: a pool of designs in a round robin, one against one, every pair on N seeds in each starting
  position (no mirror matches), under their own behaviors, at the matrix's 100 m and 90 s. The pool is designs given
  on the command line, the best elites of an evolution run file (`--run`, `--best N`: under their evolved
  behaviors), or a pool file; the default pool (`crates/core/arena/report/pool.ron`) is the 15 content ships that fly
  one against one (no bosses, stations or drones), 16 seeds: 3360 fights, about 5 s on this VM's 4 cores.
- **Ratings**: Bradley–Terry fitted to the arena scores (each fight's score counts as that share of a win, so a draw
  is half a win each), shown on the Elo scale (400 points = odds of 10 to 1), the pool's mean 1500. One virtual drawn
  fight (*placeholder*) is added to every pairing so a design that won or lost everything still gets a finite
  rating.
- **Top designs**: the best half of the pool by rating, rounded up (*placeholder*).
- **Per part** (every part of the content): the designs that carry it, its **pick rate** (the share of the top
  designs that carry it), its **win rate when used** and **score when used** (all fights of the designs that carry
  it; the pool's mean score is 0.5, since a fight's two scores add to 1).
- **Flags** (*placeholders*): **DOMINANT** picked by at least 50 % of the top designs and scoring at least 0.60 when
  used; **UNDERUSED** carried by a design of the pool but picked by under 20 % of the top designs; **NEVER PICKED**
  carried by no design. A part in every design scores 0.5 and is never DOMINANT, so logic cores and reactors that
  everything needs do not flag.
- **Form**: `arena::report` (no I/O, deterministic: ordered sums, its own `ln` from `+ − × ÷`; wasm-safe), and
  `arena report` printing two Markdown tables and the NEVER PICKED line, `--markdown` and `--ron` to write them. A
  report changes nothing: it is input for a person deciding a content change (coop-arena 3.9).

## Consequences

- The first report (`docs/results/arena-report-2026-10-20.md`) mostly measures factions: the content ships' parts
  are faction-bound, so the automatons' parts flag UNDERUSED because the automaton ships are the weakest. Reports
  about parts need pools from one palette: an evolution run's elites (`--run`) or hand-made archetypes in a pool file.
- The thresholds are placeholders; they want revisiting once a report over evolved designs exists.
- The outer loop (coop-arena A7) can compare reports before and after a proposed change.
