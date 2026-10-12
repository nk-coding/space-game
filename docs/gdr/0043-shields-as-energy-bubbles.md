# GDR-0043: Shields are drawn as glowing energy bubbles

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (the look); details marked "agent default" are the shield agent's, for review
- Supersedes: the "dotted shield rings" rule of [GDR-0007](0007-ship-style-2a.md) (repeated in [GDR-0021](0021-shields.md))
- Related: [GDR-0007](0007-ship-style-2a.md), [GDR-0021](0021-shields.md), [ADR-0029](../adr/0029-shield-bubbles-batched-procedural-mesh.md), [ADR-0021](../adr/0021-no-fixed-raster-graphics.md), `docs/style-guide.md` "Ships", `art/tokens.json` `ship.shield_bubble`

## Context

After the first play-test (12 Oct 2026) Niklas said the shields should look more glowy and fancy: the dotted
line looked bad. Shields were dotted rings in `#8fb0f2` (GDR-0007), with a sprite flash on a hit and an
expanding dotted ring on a collapse.

## Options considered

- **Energy bubble** (chosen): a translucent fill that gets brighter towards the rim (a soft glow), a slow shimmer
  moving over it, a bright ripple spreading from each hit point, flickering when the shield is low, and
  shattering when it collapses (the bubble breaks up when it empties, GDR-0021).
- **Hex lattice**: an almost invisible hex-cell skin that lights up around hits. Reason not recorded.
- **Plasma arcs**: a glowing rim with crawling electric arcs. Reason not recorded.

## Decision

- Every bubble (one per powered emitter, GDR-0021) is a glowing energy bubble in the shield colour `#8fb0f2`:
  - the fill is almost clear in the middle, so the ship's tiles stay readable, and brighter towards the rim; a
    crisp rim line sits over a soft glow;
  - a slow shimmer (drifting brightness, faint veins) moves over it;
  - every hit sends a ripple across the bubble from where the shot entered, with a flash there and light
    running along the rim;
  - when it is low it flickers and its rim crackles;
  - when it empties it shatters: a pop, cracks across the membrane, fragments going dark one by one, rim shards
    flying out;
  - a bubble that is down is not drawn.
- Agent defaults: "low" is below 30 % of the capacity; a bubble that comes back up grows in from 85 % of its
  radius, one that loses power or its emitter fades out in about 0.1 s; overlapping bubbles of one ship do not
  add up (the brighter one shows, so there are no bright lenses, but both rims stay visible because the bubbles
  stack in the rules); a beam that hits every tick sends a ripple about every 0.16 s.
- Exact values: `art/tokens.json` `ship.shield_bubble`. The debug-shapes mode hides the bubbles.

## Consequences

- The bubbles are a procedural shader, all of them in one draw call ([ADR-0029](../adr/0029-shield-bubbles-batched-procedural-mesh.md)).
  The sprite effects for shield hits and collapses are gone.
- `ship.shield_ring` in the tokens became `ship.shield_bubble`; the style guide's shield line changed.
