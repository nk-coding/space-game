# GDR-0053: Drydock v1 defaults: frames, tools, mirror, keys and test flight

- Status: Accepted (provisional, lead)
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the wave-1 drydock agents
- Related: [GDR-0051](0051-drydock-v1.md), [GDR-0052](0052-drydock-stats-full-use.md), [GDR-0006](0006-ship-scale-no-fixed-cap.md) (frames), [GDR-0008](0008-ui-ship-mind-interface.md), [ADR-0030](../adr/0030-drydock-architecture.md), `docs/drydock.md`, `docs/client.md` "Drydock", `docs/sim.md` "Design report (drydock)"

## Context

GDR-0051 fixed the scope of Drydock v1; the mockup and the concept board name the tools (paint, fill, erase,
select, rotate, mirror, bevel) but not what each does in detail, how big the frames are, or how test flights
behave. Building v1 needed answers; these are the defaults, collected for review.

## Decision

Frames (content, `content/base/frames.ron`, placeholders): **frigate 24×24, cruiser 40×40, capital 64×64**; a
frame side is at most 128. The drydock opens with the whole frame in view (100 %).

Tools (`data::edit`, `docs/drydock.md` has the exact rules):
- **Paint** (B): an empty cell becomes an armour block in the current material; a hull part takes the material.
  Machines are placed one per click with their footprint.
- **Slopes orient themselves to the hull**: the orientation with the most joined sides wins, then an open face,
  then a face leaning away from the centre line and forward. Clicking a slope again with the same part turns it;
  with another material it repaints it.
- **Bevel** (Shift+B, a mode): painting an outer corner (two adjacent sides touching the hull, the other two
  open) places a 1×1 slope instead of a block.
- **Fill** (G): drag a rectangle; its empty cells become armour blocks. Fill never bevels.
- **Erase** (X), **select** (V: move, rotate, flip), **rotate** (Q/E): a part turns about its centre (a beam
  keeps its muzzle cell).
- **Mirror** (M, on at the start): the axis is the frame's vertical centre line; every edit also applies to the
  mirror image; a mirror edit that doesn't fit is skipped and the primary edit still happens.
- **Undo/redo**: 256 steps; a drag is one step.

Keys and flight:
- **T** switches between the drydock and a test flight everywhere; the tuning panel moved to **U**.
- **C** toggles raider waves in flight; off removes the raiders at once (their wrecks stay).
- In the drydock, 1–9 pick the armour material.
- A test flight starts the design at the origin; Shift+R restarts it; losing every vessel returns to the drydock
  after about 3 s. A plain sandbox start (not from the drydock) keeps re-forming the ship.

Palette: every part and armour material in content is offered, the automaton ones included (GDR-0051:
everything is available in v1).

Design report (placeholders): test flight is blocked only by a missing logic core on the main piece, unpowered
cores or no thrust; everything else is a warning. Device load classes on the dev VM's native build: Light up to
2.5 µs per tick, Moderate up to 10, Heavy up to 40, Too heavy above.

## Consequences

- Open for Niklas: whether automaton parts and plate should stay hidden until salvage unlocks them; fill as a
  rectangle versus a bucket fill; bevel as a mode versus a one-shot "bevel the outline" command; the frame sizes.
- The keys move with a later key-binding screen; touch gestures come with the phone layout.
