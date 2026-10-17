# GDR-0066: Phase 7 wave 1 defaults: salvage values and fragments, cargo and collecting, the career model, the home screen

- Status: Accepted (provisional, lead); superseded in part by [GDR-0076](0076-spilled-cargo-becomes-fragments.md) (a shrinking hold's items spill as loose items instead of vanishing; 2026-10-14) and by [GDR-0079](0079-review-of-lead-defaults-14-oct.md) (collecting up to 8 m/s, loose items last the voyage); core matter and the take-first order confirmed there
  2026-10-16 (Niklas): the `rim_skiff`'s two shield emitters moved forward into row 1 and the two steel blocks there into
  the rear row: from the rear row their bubbles left the whole front row bare, both pulse lasers too; now they cover
  it and the stern is the bare end (the same day as the shields fix in [GDR-0021](0021-shields.md)'s Notes).
  2026-10-17: Accepted (Niklas, 17 Oct 2026).
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the wave-1 agents (tracks 7A–7D)
- Related: [GDR-0062](0062-first-playable-loop.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0065](0065-phase-7-screens-and-escort-action.md), [ADR-0033](../adr/0033-career-model-save-and-voyage-director.md), `docs/content-model.md`, `docs/sim.md` "Salvage", `docs/career.md`, `docs/client.md` "Home"

## Context

Wave 1 of Phase 7 built the content schema (7A), salvage in the sim (7B), the progression model (7C) and the home
screen (7D). Each needed choices nobody had decided; they are collected here for review. All numbers are
placeholders.

## Decision

Content (7A):
- A part's `salvage` block gives research data of one family plus `materials` (an intact part) and `fragment`
  (what a destroyed one leaves); cores give 1 programmable matter either way. On armour materials the block is per
  cell of hull (slopes count half); hull parts carry none of their own. Research data is fractional, materials
  whole numbers.
- Family colours reuse the system colours (energy and kinetic both use the weapons colour; hull is neutral).
- A start's ship may use no locked parts and must fit an open frame; locking every frame is an error.
- `rim_skiff` (the starter) is `sample_early` with a gyro pair and two radiators: it turns 90° in about 0.67 s,
  faster than the Runner (1.5 s); 7J/7K may slow it.

Salvage in the sim (7B):
- Collecting every 10 ticks, reach 5 m hull to hull; only the player collects; fragments live 60 s and have no
  physics body. "Best first" means build points per cargo unit (hull parts include their material's factor);
  cargo units are a part's footprint cells.
- A shrinking hold drops the lowest-value items first (they vanish, they are not spilled). A wreck's cargo is
  taken before its parts, the nearest loose fragment before both. A collector may strip its own debris; a
  split-off vessel starts with an empty hold.
- `spawn_wreck` removes a share of parts uniformly (cores included) without fragments.
- In the sandbox the player collects too, so every scenario's fights changed once (hashes still identical across
  platforms).

Career (7C):
- The save records `underway` while a voyage runs: a save loaded with a voyage underway counts it as lost
  (GDR-0062). The log counts launched, recalled, lost and items salvaged (no kills yet).
- An unknown start in an old save is replaced by the content's first start; unknown ids are dropped with a note.
- A ship fits if its blueprint size fits an unlocked frame; "no cargo" is checked over the whole fleet (a warning).

Home (7D):
- M goes home from flight (Esc unlocks the target); D, B and T are home's keys (T = test range). The sandbox's
  title is TEST RANGE. Disabled rings say COMING SOON until their screens exist. `THE SHIPYARD · RIM` is in code
  until content has place names.

## Consequences

- All of these are easy to change; the balance pass (7K) will revisit the numbers.
