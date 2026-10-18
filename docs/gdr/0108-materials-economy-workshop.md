# GDR-0108: The materials economy: one WORKSHOP, parts cost materials and fit from its stock, intact salvage is rare and is kept or dismantled for a share

- Status: Accepted (Niklas, 18 Oct 2026) for the decisions; Accepted (provisional, lead) for the numbers and the
  details below marked *lead*
  2026-10-18, later (Niklas: quantities and dismantling stock; the details provisional, lead): stock parts can be
  dismantled (`Career::dismantle_stock`) for the same `dismantle` share of the recipe as a hauled part (40 %) but
  **no research data** (the workshop's own parts teach nothing, and data from fabricate-then-dismantle would turn
  materials into research), and **never below what the fleet carries** (refused, `FleetNeeds`; the pill is
  disabled with the reason, rather than a warning, since a short fleet cannot launch). A part without a recipe is
  not dismantled (it would give nothing). The stock no longer only grows. MAKE and DISMANTLE (and KEEP / DISMANTLE
  of an intake part held more than once) choose a count in a strip under the row: − n + (shift: five), MAX, the
  total cost or return, then confirm or cancel; it opens at the suggestion, the fleet's shortfall or one.
  Screenshots `docs/results/workshop-count-make.png`, `workshop-count-dismantle.png`.
- Date: 2026-10-18
- Decided by: Niklas (the five decisions); lead session (names, numbers, the rules marked *lead*)
- Related: [GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0031](0031-materials-and-fabrication.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0071](0071-shipyard-home-long-u.md), [GDR-0082](0082-drydock-layout-v2-and-part-availability.md), [GDR-0089](0089-play-test-15-oct-speed-salvage-waves-station.md), [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md), [GDR-0104](0104-scenery-rocks-filler-landmarks.md), `docs/career.md` "The workshop", `docs/results/materials-inflow-2026-10-18.md`

## Context

GDR-0063 left parts unlimited until fabrication existed, and dismantled the whole haul at home automatically. Two
dark buildings waited at home for it (the fabricator and the salvage bay, `LATER`). Niklas (18 Oct 2026): salvage is
too generous (so much material that collecting stops being a challenge), and the economy of GDR-0031 should come now.

## Decisions (Niklas, 18 Oct 2026)

1. **One building** for parts: the fabricator and the salvage bay become one, the **WORKSHOP**; the other slot is
   freed.
2. **Parts cost materials** (GDR-0031 in full, ending GDR-0063's unlimited parts): recipes in content, logic cores
   take programmable matter, workshop upgrades open part tiers and race parts; fabrication only at home; the drydock
   shows cost and stock; a fleet whose parts are not in stock cannot launch (what is missing is shown; the workshop
   fabricates it).
3. **Per hauled part: keep or dismantle**, with a sensible default so it is not tedious.
4. **Less generous salvage**: most parts break when a ship dies or is stripped, only a small share survives intact;
   dismantling returns only part of a part's materials.
5. **Niklas's live career keeps working**: migrated with a stock covering his designs and escorts.

## Options considered (lead)

- Which slot stays: the salvage bay's, under the bottom wall beside the moorings where the shuttles berth (chosen:
  the haul comes in there), or the fabricator's on the top wall between research and the LOG (freed: room for a later
  building, such as a refinery for GDR-0104's ores).
- What a launch does with parts: **fitted from stock without consuming it: a fleet launches when the stock holds
  every part its ships carry together; the parts come back with the ship, and a lost ship's bonded parts re-form at
  home** (chosen: GDR-0029's mild death, "rebuilding is free"); parts consumed at launch and returned at home, lost
  with a lost ship (rejected: it would contradict GDR-0029 and make a loss a repair bill, GDR-0030).
- What the stock limits: the launch (chosen; designs may hold more than the stock, they plan ahead), or every
  placement in the drydock (GDR-0082's counts in the editing model; rejected: fabricating first, then placing, is
  tedious; the lab keeps its counts).
- Hull: plating not counted for now (chosen; armour materials stay gated by research, and dismantled hull gives a
  share of its armour material per cell), or plating as stock per material (later, recorded below).
- Where intact or broken is decided: when a part is stripped, in the sim (chosen: the hold shows it, a broken part
  is worth half when a hold sheds), or at home when the haul is unloaded.
- Workshop upgrades: facilities `within` the workshop, opened by research nodes (chosen: no new content kind), or a
  kind of their own.

## Decision details

- **The WORKSHOP** (*lead* name; content facility `workshop` with `workshop` rules) is a layer over home at the old
  salvage bay's place (W): left the haul's intact parts to sort (KEEP, DISMANTLE, one at a time, and SORT AS
  SUGGESTED for all), right fabricating (what the fleet is short of with FABRICATE MISSING, and every unlocked part
  with a recipe: stock, recipe, MAKE), the materials in store top right. Its bays glow gold while parts wait.
- **Recipes** (`Part::recipe`: materials and an optional `needs` upgrade) on every non-hull part. Materials: steel and
  programmable matter (logic cores 2, automaton cores 3), three new fabrication resources (copper, ceramic, lattice
  crystal; `materials/fabrication.ron`), the races' own matter (automaton plate, grown tissue). Amounts about twice a
  part's build points (*lead*, placeholders).
- **Workshop upgrades** (GDR-0031's hub upgrades, the minimum set): the **fusion workshop** (fusion reactor, torch
  drive; installed by researching the fusion reactor), the **growing vat** (the Grown's parts; installed by Living
  weapons), the **automaton forge** (the automatons' core and cannon; nothing installs it yet, so they come only
  from intact salvage). The rest of GDR-0031's list (smelter, kiln, cryo plant, electronics bench, magnetics lab,
  fuel works, munitions bay, the other races' upgrades, the precursor forge) waits for parts and materials that need
  them.
- **The stock** (save format 5: `stock`, `intake`): a new game has its start ship's parts in stock and the start's
  `materials` (ceramic 2, copper 4, crystal 2, steel 4: a spare laser and a thruster). Readiness gains
  `MissingParts { part, need, have }` (after the locked parts, before the cores); the drydock's palette rows show the
  stock less what the design holds (coral when short), its inspector the recipe and the stock, its status line and
  the map's checklist what the fleet lacks. Free mode (`?unlock=all`) ignores the stock.
- **Breaking** (sim, `SalvageTuning::intact_chance` 0.15, *lead*): a stripped part that is not hull comes away
  intact with that chance, one draw on the debris stream; else as `Item::Broken` (same hold units, half the value).
  A chance of 1 draws nothing (the sim before this record).
- **At home** (`career::unload`): a broken part gives its research data and the workshop's `broken` share (25 %,
  *lead*) of its recipe; hull its armour's data and 25 % of a unit of its armour per cell; fragments as before; an
  intact part that is not hull goes to the intake and gives nothing until sorted. Dismantling an intact part gives
  its data and `dismantle` (40 %, *lead*) of its recipe. Shares are summed over the haul (or the parts dismantled
  together) and rounded to the nearest unit.
- **The suggested sort** (`keep` 2, *lead*): keep while the stock holds fewer than two or than the fleet carries,
  dismantle the rest.
- **Migration** (format 4 to 5): a save without a stock gets, per part, the most of what its fleet (working design
  and escorts) carries together and of what any one saved design carries; it is written at once, with a note (THE
  WORKSHOP). Tested with a fixture of a mid-game career (`crates/client/world/src/fixtures/career_format_4.ron`).

## Numbers measured (`docs/results/materials-inflow-2026-10-18.md`)

The harness's voyage mode over 200 seeds of the four slice voyages: per voyage home, the materials the haul gives
fell from the *whole* value (every part intact and dismantled for its full recipe, GDR-0031 as written) to about a
third (skiff 4.8 → 2.2 units on the yard approach, the Runner with the skiff 18.3 → 6.6; 25–45 % across the
voyages), most of it programmable matter from fragments; fabrication materials (copper, ceramic, crystal, steel) about
0.3 a voyage for the skiff's hold and 2–5 for the Runner's fleet, against 6–10 a part. Intact parts: 0–0.4 a voyage.

## Consequences

- Screenshots (1280 × 720): `docs/results/workshop-layer.png` (the WORKSHOP after a dismantle),
  `workshop-drydock.png` (a part the design holds more of than the stock), `workshop-home.png` (7 TO SORT).
- The stock grows by keeping and fabricating (and shrinks only when the player dismantles stock parts the fleet
  does not carry); nothing is lost but the haul, as GDR-0029 wants.
- The research flow is nearly unchanged (broken parts give their data at once; intact ones when dismantled).
- `SalvageSpec::materials` is gone: a part's materials come from its recipe.
- The sim's state hash changes with the intact roll (the debris stream, the new item); the determinism check passes.
- Later: hull plating as stock, installing salvaged
  parts "as is" with a bonding cost, harvesting (GDR-0104's ores), more upgrades, the inflow re-tuned after play.
