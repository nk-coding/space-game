# ADR-0012: Coordinate frames and units (1 block = 1 m)

- Status: Accepted (provisional, lead)
- Date: 2026-10-11 (frames and units); 1 block = 1 m was decided 2026-10-10
- Decided by: Niklas (1 block = 1 m); lead session (frames and units, default for Niklas to review)
- Related: ADR-0013, GDR-0006, `docs/content-model.md` "Coordinates and geometry", `docs/sim.md` "Frames and units"

## Context

Blueprints, the sim, physics engines and the Bevy renderer each have a natural frame. The drydock rules
(`drydock-model.js`) use screen coordinates with y down; physics engines and Bevy use y up.

## Options considered

Not recorded beyond the choice made. Body conventions follow Rapier and Box2D so an engine can plug in directly.

## Decision

- **1 cell (block) = 1 m** (Niklas, 10 Oct). A 60×60 ship is 60 m across.
- **Blueprint frame** (`data`): x right, **y down**, forward = up (−y). Cell `(x, y)` covers
  `[x, x+1] × [y, y+1]`; `flip` mirrors left-right before `rot` (quarter turns clockwise on screen).
- **Piece frame** (one rigid body in `sim`): x right, **y up**, metres, forward = +y; origin = the piece's centre
  of mass when the piece was created. Blueprint `(x, y)` → `(x − ox, oy − y)`.
- **World frame**: x right, y up, metres, angles counter-clockwise in radians. `Body.pos` is the frame origin,
  `Body.linvel` the centre-of-mass velocity.
- **Bevy**: `world = (x, −y)`, rotation `−rot × 90°`.
- **Units**: mass in t, thrust in kN (kN / t = m/s²), torque in kN·m, inertia in t·m²; heat is a unitless level
  relative to each part's limit. Other unit labels are placeholders.

## Consequences

- Content keeps the drydock's screen convention (a direct port of the concept rules); the sim converts once.
- Tests check that the client mapping matches the sim.
