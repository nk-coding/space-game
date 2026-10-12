# GDR-0051: Drydock v1 comes next: unlimited parts, stats as information, test-fly into the sandbox, the mockup's look

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (interactively, after the move to the new VM)
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md) (grid building), [GDR-0006](0006-ship-scale-no-fixed-cap.md) (frames, device load), [GDR-0008](0008-ui-ship-mind-interface.md) (drydock UI), [GDR-0030](0030-no-economy.md) (build points), [GDR-0038](0038-logic-cores-control-parts.md) (control load), [ADR-0030](../adr/0030-drydock-architecture.md), `docs/drydock.md`, `docs/concept/boards/ui-mockups/desktop-drydock.png`

## Context

Phase 3 (flight, weapons, behaviors, vessels) is done. The roadmap's next large items were the drydock, a
balance and feel pass, the salvage loop and closing the spike (measurements and the spike summary). Ships so far
come only from content files; nobody can build one in the game. Salvage, fabrication and research, which would
supply parts, don't exist yet.

## Options considered

What first:
- **Drydock v1** (chosen): the core of the game, and it makes every later system testable with ships the player
  builds.
- **Balance and feel pass**: the open lead defaults and fight numbers.
- **Salvage loop**: nothing to spend materials on without a drydock.
- **Close the spike first**: native load test, browser numbers, `docs/results/spike-summary.md`.

Where parts come from in v1:
- **Unlimited; build points, device load and control load shown as information only** (chosen).
- A build-point budget per frame.
- A fixed starter stock in content.

Test-fly:
- **Into the sandbox; raider waves on, a key toggles them; T again, or losing the ship, returns to the drydock
  with the design untouched** (chosen).
- A calm test range with target dummies.
- Both as two launch buttons.

Look:
- **Close to the mockup** (chosen): match `desktop-drydock.png` as closely as Bevy UI allows from the start.
- The mockup's layout with plainer styling.
- Functional first, styled later.

## Decision

- Drydock v1 is the next phase. It runs in the web client (desktop layout first; the phone layout later from the
  same pieces).
- Every part and armour material is available in unlimited numbers. The drydock shows build points, device load
  and control load as live information; nothing is enforced yet.
- Test-fly launches the design into the sandbox with raider waves on; a key toggles the waves. T again, or losing
  every vessel, returns to the drydock with the design unchanged.
- The screen follows the desktop drydock mockup closely: part palette with search and categories on the left,
  the grid in the centre with the tool pills above it, live stats and the selected part on the right, armour and
  overlay pills at the top, undo/redo, mirror, bevel, status line, blueprints, export, save and test-fly at the
  bottom.

## Consequences

- Upgrade materials, variants, spare counts and "track on map" from the mockup wait for salvage, research and
  fabrication.
- Player designs are saved and exported as blueprints in the content's ship format (ADR-0030).
- Details the lead set as defaults while building v1 (frame sizes, what fill and bevel do, keys) are recorded
  separately for Niklas to review.
- The spike summary (Phase 4) is still owed; it is done alongside.
