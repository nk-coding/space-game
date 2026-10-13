# GDR-0061: Fight balance after turning v2: raiders orbit inside the beams, the skirmisher keeps its nose on the target, shields can collapse

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively, 13 Oct 2026; proposed by the balance agent)
- Related: [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0018](0018-weapon-families.md), [GDR-0021](0021-shields.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md), [GDR-0041](0041-engine-rated-speed.md), [GDR-0043](0043-shields-as-energy-bubbles.md), [GDR-0055](0055-slower-crisper-turning.md), [GDR-0058](0058-turning-v2-defaults.md), `docs/results/balance-2026-10-13.md`, `docs/sim.md` "Sandbox fights at the new speeds"

## Context

The turning was reworked on 13 Oct 2026 (GDR-0055, GDR-0058: the Halvex Runner turns 90° in 1.5 s instead of
0.77 s, the raider in 0.57 s instead of 0.40 s) and the fight balance was not re-measured. Open items: the
`raider_basic` stand-off and orbit sit on a knife edge around the beam range, the `skirmisher` wingmen win 3–5 of
12 waves, shields rarely collapse. A headless harness (separate repo, ADR-0025) measured 10 setups over 200 seeds
each (`docs/results/balance-2026-10-13.md`). Findings (current content and sim):

- `raider_basic` orbits at 67% of its cannon's 75 m = 50.25 m, exactly where the Runner's beams (45 m from the
  nose) stop touching a raider. At 70% a passive Runner wins 0 of 200 waves instead of 30%. The pulse lasers the
  12 Oct tuning counted on cannot hit a raider the nose points at (5.5 m off the axis, 1–2% hits), and the beams
  at the unshielded nose are shot off first (1.3 of 2 per wave).
- The `skirmisher` spends a quarter of a wave in two orbit rules that turn the nose away from the target (the
  crowd rule fires against every wave of three); every switch back costs 1.5–3 s of turning since GDR-0055. A lone
  Runner fires in 12% of the time a raider is within its reach and wins 28% of the waves (3 Runners vs 6
  raiders: 18%).
- Shields collapsed in none of the baseline fights: the bubbles take 17–35% of the cannon hits, 0.40 each, and
  regenerate 1.5/s.
- A sim bug (AI ships stuck at their top speed under the whole-speed fade of GDR-0055) decides many AI-vs-AI
  fights; with a diagnostic fix the lone skirmisher wins 2% instead of 28%. The changes below are measured on
  both.

## Options considered

- **Raider orbit**: (a) **move it inside the beam reach, 67% → 58% (43.5 m)** (chosen): the raiders still shoot
  from 64 m, out of reach, for 5 s of every 9 s, then strafe through the beams; the cliff (67–70%) is 9–12 points
  away. (b) Shorter stand-off (approach 75% or 60%) or a shorter cannon (75 → 60 m): also removes the edge, but
  the raiders would shoot from inside the beams and the whole fight changes (approach 60%: no passive pilot ever
  loses). (c) Move the orbit out of reach (≥ 75%): raiders are invulnerable while circling; a passive pilot loses
  every wave.
- **Skirmisher**: (a) **delete the crowd orbit (`enemies_near(100m) >= 3 -> orbit(90%)`) and the cycle orbit
  (`target_in_range(120%) and cycle(10s) >= 6s -> orbit(60%)`)** (chosen): best in every setup on both sims (lone
  Runner 28% → 44%, 3 vs 6: 18% → 74%). (b) Crowd rule at ≥ 4 and a 2 s orbit: half the gain. (c) Keep the rules
  and wait for faster turning: not on the table (GDR-0055).
- **Shields**: (a) **capacity 6 → 3, regen 1.5 → 0.2** (chosen): bubbles collapse under sustained fire (20% of
  the passive pilot's waves, 13% of the lone skirmisher's, 21% of 3 vs 6, never in a 1v1), win rates move by
  ±2%. (b) Additionally **radius 2.3 → 3.5 m** (option B): the bubbles then cover the beams, which survive
  (beam losses 1.30 → 0.29 per wave), and the Runner becomes much stronger (passive pilot 50% → 82% wins, lone
  skirmisher 54% → 87%); also a visible change of the bubbles (GDR-0043) and a threshold (3.3 m covers nothing
  extra). (c) Lower regen only: changes nothing (bit-identical fights).

## Decision

Proposed (content placeholders, exact before → after):

- `behaviors/raider_basic.ron`: `when target_in_range(120%) and cycle(9s) >= 5s -> orbit(67%)` → `... -> orbit(58%)`.
- `behaviors/skirmisher.ron`: delete the rules `when enemies_near(100m) >= 3 -> orbit(90%) # do not get
  surrounded` and `when target_in_range(120%) and cycle(10s) >= 6s -> orbit(60%)`; summary `Attack and strafe,
  dodge while the shields recharge, and run when badly hurt.` → `Attack nose first, dodge while the shields
  recharge, and run when badly hurt.`
- `parts/defense.ron`, shield emitter: `capacity: 6, regen: 1.5` → `capacity: 3, regen: 0.2` (radius stays 2.3;
  option B: 3.5).

Measured (200 seeds, win / loss / timeout %, current sim → with the set; fixed sim in brackets): passive pilot vs
a wave of 3: 30/49/22 → 50/22/28 (34/43/24 → 53/22/25); closing pilot: 71/6/22 → 74/6/20; lone skirmisher
Runner: 28/32/40 → 54/15/31 (2/70/28 → 40/28/32); pilot + 2 wingmen: 85/0/15 → 99/0/1; 3 Runners vs 6: 18/18/64 →
74/1/24 (6/39/56 → 84/0/16); waves end sooner (lone skirmisher, contact to last kill: 77 s → 26 s).

## Consequences

- Tests that encode the old content need their numbers updated: `data/tests/behavior_edit.rs` (three tests on
  the skirmisher's six rules), `sim/tests/power.rs` (regen 1.5/s), `sim/tests/vessels.rs` (capacity 6),
  `game/src/behavior_ui/live.rs` and `screen.rs` (row labels, rule counts). `docs/sim.md` ("raider_basic",
  "Sandbox fights") and the determinism scenario counters change; hashes change once.
- The skirmisher no longer strafes; it dodges only when its shield is down (now a real event).
- Open, not decided here (sim, see the report's "Sim issues"): the top-speed bug (fix it: goals capped at the top
  speed and a fade continuous in the push angle); engine-less AI vessels never aim; the lock and `face` lag
  moving targets (no rate feed-forward; worse since GDR-0055); off-axis fixed guns cannot hit small targets
  (GDR-0018); split-off vessels flee at once. The gyro-less sample ships still lose most duels against one
  raider under turning v2 (Sample early 30% wins).
