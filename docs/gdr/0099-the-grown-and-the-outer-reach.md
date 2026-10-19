# GDR-0099: The Grown and the outer reach: heart-fed regrowth, rammers with thorn guns, calm until provoked, regrowing armour and a ram prow first, hot clusters, a precursor guardian at each gate inward

- Status: Accepted (Niklas, 17 Oct 2026); built (phase 1 on 17 Oct, phase 2 on 18 Oct 2026)
  2026-10-17: phase 1 built, the mechanics and parts (regrowth, healing tissue, the ram prow, the thorn gun, wary
  sites, hot stars; their rules and numbers lead defaults in [GDR-0102](0102-grown-mechanics-and-parts-defaults.md));
  phase 2 (the Grown's ships, behaviors, groves, cluster types, research family, guardians and bosses) not yet.
  2026-10-18: phase 2 built: the Grown's rammer, thornback, grove guardian and elder, groves, the outer reach's
  cluster types with rim stragglers near the border, the Grown research family, the bastion at each outer gate and
  the elders at sites of their own; lead defaults in [GDR-0103](0103-the-grown-in-play-defaults.md) (A6 measured: a
  Runner with one escort clears a grove run 55 %, an outer gate 52 %).
  2026-10-18: the Grown's own materials (lead, content batch; "Open" here and in GDR-0103): heartwood (a destroyed heart leaves one, a heart's recipe and research take it, an elder's trail gives three) and grove spores (in the living parts' recipes, so stripping a grove's wrecks gives them back); spores line a new radiator, the spore-lined radiator, through a fourth Grown node (`spore_lining`).
  2026-10-18: A7's guardian at the middle band's gates (no longer the Warden as a stand-in): the Keeper, a step above the bastion ([GDR-0112](0112-the-keeper-guards-the-middle-bands-gates.md)).
  2026-10-19: the inner band's gates have their own guardian, the Spire, a step above the Keeper ([GDR-0116](0116-the-spire-guards-the-inner-bands-gates.md)).
- Date: 2026-10-17
- Decided by: Niklas (interactively, 17 Oct 2026; the recommended option on every question)
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0027](0027-races.md), [GDR-0031](0031-materials-and-fabrication.md), [GDR-0022](0022-heat-per-part.md), [GDR-0021](0021-shields.md), [GDR-0024](0024-tech-trees-no-mk-levels.md), [GDR-0025](0025-research-from-salvage.md), [GDR-0047](0047-ramming-crushes-inward-bumps-free.md), [GDR-0073](0073-slice-tuning-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md), [GDR-0100](0100-fight-balance-targets.md)

## Context

Clearing the Warden opens the rim's first gate into the outer reach (GDR-0098), but the outer reach is still a stub
(GDR-0094: the rim is real, the other bands are stubs). The concept gives it only a line or two:

- GDR-0026: the journey goes inward from the rim through the outer reach, gated by distance, race territory and
  hazards (stellar heat in the outer reach); a precursor guardian sits at every band gate; bosses include giant ships.
- GDR-0027: the outer reach is the Grown's, a wary race of organic hulls that regrow and love ramming with living
  prows; its boss is giant Grown ships; each race teaches its own tech branch.
- GDR-0031: race-specific sources give race materials; materials follow the hazards.
- GDR-0022: heat is per part and lost to space through open sides; radiators vent the most.

Open were how regrowth works, how the Grown fight, what "wary" means in play, what the outer reach teaches first,
what makes the band itself different, how hard it is, and who guards its way to the middle reach.

## Options considered

Niklas chose the recommended option on each of the seven questions. The other options offered are not recorded.

## Decision

- **Regrowth is heart-fed** (A1): a Grown ship's lost blocks slowly re-form from their living neighbours while a heart
  part lives. Killing the heart stops it.
- **Rammers with thorn guns** (A2): living prows charge and ram; short-range thorn spines cover the approach. Thorns
  are kinetic, so half their damage passes a bubble (GDR-0021). They reuse the existing ram rules (GDR-0047) and
  kinetic rules; no new damage model.
- **Wary means calm unless provoked** (A3): a Grown site does not wake on sight, unlike automatons. Shooting it,
  ramming it, scanning nearby or stripping its wrecks wakes it.
- **First tech** (A4): regrowing tissue armour (heals slowly) and a ram prow part. Grown salvage feeds a new Grown
  research family (GDR-0024, GDR-0025).
- **The outer reach** (A5) has heat zones and the Grown:
  - some clusters near hot stars heat the parts exposed to space, so radiators and heat research matter
    (GDR-0022);
  - its sites are Grown groves, with rim (automaton) stragglers near the border.
- **Difficulty** (A6): a Runner-class flagship with 1–2 escorts and the rim researched can handle the outer reach.
- **Gates to the middle reach** (A7): a precursor guardian at each gate (GDR-0026); giant Grown ships are separate
  boss sites (GDR-0027), not gate guardians.

## Consequences

- Implementation comes later; nothing here is built. It needs, roughly:
  - a heart part and the regrowth rule in the sim;
  - the thorn spine weapon;
  - a wake rule on provocation beside the wake-on-sight one (scans already wake automatons, GDR-0080);
  - tissue armour and the ram prow as parts, and the Grown research family;
  - hot-star heat in cluster types;
  - the outer band's content in the generator (groves, stragglers, gates and their guardians, boss sites).
- All numbers (regrowth rate, thorn reach, heat, wake radii) will be placeholders in content.
- The difficulty target is measured the way GDR-0073 measures the slice (the headless voyage runner over seeds),
  with the encounter counts as the first knob ([GDR-0100](0100-fight-balance-targets.md)).
- Open: the Grown's materials and the outer reach's other materials (GDR-0031), the guardians' designs, how a grove
  looks.
