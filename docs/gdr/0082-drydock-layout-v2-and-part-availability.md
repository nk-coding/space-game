# GDR-0082: Drydock layout v2 (every part in sections, armour swatches on the left, tools in the bottom bar, icons and a ⋯ menu) and part availability (unlimited for now)

- Status: Accepted
  2026-10-18: the career fills no counts in the editing model ([GDR-0108](0108-materials-economy-workshop.md)): designs are not limited by the workshop's stock (they plan ahead); the launch is, and the palette rows show the stock less what the design holds. The counts stay for the lab's runs.
  2026-10-18, evening (Niklas: "the power output looks shit"; a part in the list should tell about itself on hover):
  SELECTED PART's stats are a two-column stat block, a label over its value on one line (never wrapping into it),
  RECIPE and IN STOCK across both columns; the pointer resting on a palette row shows a hover card beside the column
  (icon, name, size and category, the same stat block with the recipe, the bonuses). The left column is the kit's
  grouped list, shared with the workshop (search, chips, sections unchanged). `docs/client.md` "Part lists and the
  hover card".
  2026-10-18, late (Niklas: the card "should not cover the bottom area"): the hover card stays between the top bar and
  the bottom of the list (moved up to end there; closer rows, then three columns for a part too tall for that room);
  RECIPE reads like the other values; a list's texts cut by its scrolled edge go whole; the category chips get a
  small step ring at each end while they don't fit.
- Date: 2026-10-14
- Decided by: Niklas (in chat and interactively, with the draft `docs/plans/mockups/drydock-left-column.html`)
- Related: [GDR-0051](0051-drydock-v1.md), [GDR-0053](0053-drydock-v1-lead-defaults.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0031](0031-materials-and-fabrication.md), [GDR-0081](0081-drydock-editing-bevel-rotate-replace.md), `docs/drydock.md`

## Context

Niklas, 14 Oct 2026: the drydock's left palette shows only one category at a time; the armour material sits in the
top bar; the tool row sits above the grid; the bottom bar holds many rarely used buttons. He also noticed that parts
are unlimited, though the concept says parts are made and bonded (GDR-0029, GDR-0031). That concept stands; the
career slice deferred it on purpose (GDR-0063: an unlocked part is unlimited until fabrication exists).

## Options considered

- **Category chips**: jump to their section in one list (chosen, as one line that scrolls sideways, Niklas: "should
  take less space"); collapsible sections; no chips.
- **Armour picker** (from the draft): A, a swatch strip with the current name above it (chosen); B, a dropdown; C,
  materials as a section of the list. About 8–12 materials are expected in the end (GDR-0031: 30+ materials, a few of
  them armour), so a row of named pills would not scale.
- **Bottom bar**: icon buttons plus a ⋯ menu (chosen); everything rare in one ⋯ menu.
- **Availability**: build the plumbing now with every part unlimited (chosen, Niklas: "if just not implemented, do
  so with an availability of infinite for now"); wait for fabrication.

## Decision

- **Left column**, top to bottom: search; `ARMOR · <name>` with a strip of material swatches (scrolls sideways past
  about 8; hover gives name and stats; keys 1–9 pick); one line of category chips that scrolls sideways and jumps to
  its section (the chip of the section in view lights); then **every part in sections** (a heading per category with
  its count). Search filters the whole list.
- **Bottom bar**: undo and redo as icons; the tools (PAINT, FILL, ERASE, SELECT, ROTATE) move here from above the
  grid, with MIRROR and BEVEL; when the bar is too narrow the tools fold into one dropdown (`PAINT ▾`) and mirror and
  bevel into icons; the status line shortens; SAVE, BLUEPRINTS and EXPORT become icon buttons with tooltips
  (name and key); rarer actions go behind a ⋯ menu; BEHAVIOR and TEST-FLY stay as words.
- **Part availability**: the career holds how many of each part are available; every part is **unlimited** for
  now. The palette shows the count (∞ today), and the editing model refuses a placement when none is left. Fabrication,
  spares and bonding (GDR-0029, 0031; `docs/plans/salvage-research.md`) fill the counts in later.

## Consequences

- The armour tones are close (steel, composite and reactive are all near-black), so the swatches carry a light
  outline and the current name always shows.
- The top bar keeps the overlays only; the area above the grid is free.
- When fabrication comes, the counts become real; the questions it leaves stay in `docs/plans/salvage-research.md`.
