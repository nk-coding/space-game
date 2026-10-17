# GDR-0063: Salvage and research v1: fly over wrecks, the haul is dismantled at home into research data, unlocks are unlimited

- Status: Accepted
  2026-10-18: unlimited parts end ([GDR-0108](0108-materials-economy-workshop.md), Niklas): parts cost materials and a fleet fits from the workshop's stock; the haul is no longer dismantled whole and automatically: most stripped parts break (dismantled at once for their data and 25 % of their recipe), intact ones wait in the workshop to be kept or dismantled; `salvage.materials` is gone (a part's materials come from its recipe).
- Date: 2026-10-13
- Decided by: Niklas (interactively)
- Related: [GDR-0024](0024-tech-trees-no-mk-levels.md), [GDR-0025](0025-research-from-salvage.md), [GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0030](0030-no-economy.md), [GDR-0031](0031-materials-and-fabrication.md), [GDR-0051](0051-drydock-v1.md), [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md), [GDR-0062](0062-first-playable-loop.md), `docs/plans/first-playable-loop.md`, `docs/plans/salvage-research.md`

## Context

The first playable loop (GDR-0062) needs a way from wrecks to new parts. GDR-0025 (research from salvage), GDR-0029
(bonded matter) and GDR-0031 (materials and fabrication) describe the full design, but fabrication, recipes and an
inventory are a large system. A raider's only core is also its power, so a raider becomes salvage only when that
core is destroyed: fights never yield intact cores, although GDR-0031 names automaton cores as the first source of
programmable matter.

## Options considered

- What limits building: **unlocks only; researched parts are unlimited, the frame and the mind capacity limit a
  design** (chosen), unlocks plus a build-point budget per frame, or material costs now.
- Starting kit: **the early-game kit (steel, fission, pulse lasers, thrusters, ...); composite, fusion, beam lasers,
  the torch drive and the bigger frames come from research** (chosen), today's kit, or a bare minimum.
- Collecting: **fly slowly over a wreck; the ship strips its parts into its cargo, best first, until the hold is
  full** (chosen), automatically when a site is cleared, or a tractor beam part.
- The haul when the last ship is lost: **it stays in the wreck's cargo for the rest of that voyage; an escort can
  scoop it** (chosen), half comes home, or the wreck persists in that cluster.
- Turning the haul into research: **dismantled automatically at home into research data per tech family; a node
  unlocks at once when paid** (chosen), dismantle or keep per part, or timed research.
- Programmable matter: **a destroyed core leaves a small programmable-matter fragment to scoop; intact cores still
  come from derelicts** (chosen), only pre-wrecked "core graveyards", or both with small fragments.

## Decision

- Research unlocks parts, materials, frames and mind capacity; an unlocked part is unlimited in the drydock (the
  career's drydock; GDR-0051's free drydock stays for test flights and development). The content's `locked` flag
  (GDR-0054) marks what a new game doesn't have; research clears it per player.
- A new game has the early-game kit and a small starter ship; the Halvex Runner's parts become the mid-game goal.
- Salvage v1: a player vessel strips the salvage piece in reach part by part, highest value first, into the cargo of
  its ship (cargo bays and holds), until full. Cargo belongs to the ship; a lost ship's cargo stays in its wreck for
  the rest of the voyage; whatever is left behind is gone when the voyage ends. No repairs during a voyage.
- At home the haul is dismantled into research data per tech family (families as content, following GDR-0024) and
  materials; research nodes are content with a cost in data and materials and unlock at once when paid.
- A destroyed logic or automaton core leaves a small programmable-matter fragment that can be scooped like a part.

## Consequences

- Fabrication, recipes, part costs in bonded matter, spare counts, "install as-is", cutting parts free, tractor
  beams, scans and timed research wait (`docs/plans/salvage-research.md`); GDR-0025, 0029 and 0031 stay the target.
- Cargo parts become a design trade-off. Salvage, cargo and fragments live in `sim` (deterministic, in the state
  hash); unlocks and research in a pure `data` model (ADR-0033).
- The free drydock and the sandbox hide the locked parts unless a development switch (`?unlock=all`) is on.
