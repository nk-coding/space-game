# GDR-0073: Slice tuning v1: smaller packs, a gentler first voyage, research costs about halved

- Status: Proposed
- Date: 2026-10-13
- Decided by: balance agent (proposal, tracks 7J/7K)
- Related: [GDR-0061](0061-fight-balance-after-turning-v2.md), [GDR-0062](0062-first-playable-loop.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0064](0064-lock-feed-forward-and-ai-flight-fixes.md), [GDR-0066](0066-phase-7-wave-1-defaults.md), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), `docs/results/slice-balance-2026-10-13.md`, `docs/plans/first-playable-loop.md` 3.5, `docs/sim.md` "The headless voyage runner"

## Context

The slice's content (track 7J's draft) had not been measured. With the headless voyage runner (its pilot made closer
to sensible play first: it stops short of a sleeping pack, keeps its lock, finishes off disarmed enemies and strips
the best parts first) and 60 seeds per setup, almost nothing could be cleared: the starter `rim_skiff` cleared the
first voyage in 43% of the runs and nothing after it; the Halvex Runner (the mid-game target) cleared 37% of the first
voyage and none of the others; the Warden with a Runner and an escort fell in 2 of 60 runs. Fights decide it: in the
voyages' geometry (a pack that wakes together and charges) one ship of the slice beats two raiders about 90% of the
time and three about half the time, and damage stays for the whole voyage. A run brought home about 9 research data
spread over eight families (the starter's hold is 8 units), so the plan's node costs (8–30 per family) needed 5–20
runs each.

## Options considered

- **Encounter counts and site lists** (chosen): fewer and smaller packs; the hunters and the Warden make the later
  voyages harder instead of raw raider numbers. Content only, each step measured with the same seeds.
- **Stronger ship-mind ships or weaker raiders** (a bigger starter, raider cannon or stand-off numbers): would move the
  sandbox and the fight balance of GDR-0061 as well; not needed for the curve.
- **Research costs** about halved (chosen) or **salvage data doubled**: the same effect; the costs are the knob the
  research content marks as a placeholder, and data per part stays "about its build points".
- **Sandbox wave**: a bigger wave (4: the passive Runner wins 88%) or raider numbers (orbit 64%: passive 51%, but
  4–6 points from the cliff GDR-0061 moved away from). Not changed here: the wave size is client code
  (`raiders_per_wave`), and the test range now flies the player's own design (the starter wins 50% of waves of 3 and
  10–24% of waves of 4). Open for Niklas.

## Decision

Content placeholders, exact before → after:

- `voyages/rim.ron`: Shipyard approach's goal `raider_pack` → `raider_pair`. Halvex Drift: `raider_pack` ×2 →
  `raider_pair` ×1. Corran Reach: `hunter_pack` ×2 and a `raider_pack` → one `hunter_pack`. The Warden's gate:
  `raider_pack` ×2 → ×1. Expected minutes: Halvex Drift (10, 15) → (6, 10), Corran Reach (12, 15) → (5, 8), the
  Warden's gate (10, 20) → (6, 10); Shipyard approach stays (6, 8).
- `encounters/automatons.ron`: `raider_den` 4–5 raiders → 2–3; `hunter_pack` 2 hunters + 0–1 raider → 1 hunter + 1
  raider; `hunter_den` 2–3 hunters + 1–2 raiders → 2–3 hunters; `precursor_wrecks` 0–2 raiders → 0–1;
  `core_graveyard` 1–2 raiders → 0–1; `the_warden` 2–4 raiders → 1–2. Summaries of the den, the hunter pack and
  the hunter den follow the counts.
- `research/rim.ron`: cannon kinetic 12 → 3; composite hull 20 → 8; beam energy 18 → 10; fusion power 20 → 10;
  torch drive propulsion 20 → 10; tamed core logic 8 → 3 (programmable matter 1); mind expansion I logic 10,
  programmable matter 2 → 3; mind expansion II logic 20 → 16, programmable matter 4 → 5; cruiser drydock hull 30
  → 20.
- Unchanged: the starter's layout, the hunter and Warden designs, the escort template, wake radii, site spacing,
  wreck damage shares, salvage values, the sandbox wave.

Measured (same 60 seeds and pilot; goal cleared and home, per run): the starter 43% → 58% on the first voyage, 0 →
37% on the second, 0 → 15% on the third; the Runner 37% → 60%, 0 → 67%, 0 → 45%, the Warden alone 0 → 20%; the
Runner with a starter as escort at the Warden 3% → 42% (two Runners 70%). A home run of the starter brings ~10 data
and 2 programmable matter (a skiff with a cargo hold ~26 on the second voyage), so two first voyages pay one node and
the Runner's four nodes take about four second voyages with a hold. The evening estimate is about 2.7 h (scripted
times doubled, plus overhead).

## Consequences

- Every voyage is harder than the one before for the starter (58 / 37 / 15 / 3%) and from the second on for the
  Runner (60 / 67 / 45 / 20%); the Warden needs a mid-game ship and an escort for even odds.
- The hold is the first upgrade: the haul is hold-limited (8 units on the starter), and the costs are set for a
  player who adds a cargo hold after the first voyage; without one the second step of research takes twice as many
  runs.
- Open (numbers in the report): the reverse-engineered cannon is a step back for the scripted pilot (a cannon starter
  clears 12% of the first voyage against 58% with pulse lasers); a disarmed ship cannot outrun a raider of the same
  top speed nor recall within 200 m of it (8–30% of runs end that way); the Runner's nose beams and fusion reactor
  make it fragile against three raiders at once; the sandbox wave size.
- Tests that encode the content changed with it (`data`, `sim`, `content-tool`, and the client's map and research
  tests); the determinism scenario `voyage` (it flies the first voyage) changes its stream once.
