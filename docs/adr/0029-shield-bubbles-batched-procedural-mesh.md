# ADR-0029: Shield bubbles are one batched world-space mesh with a procedural WESL material and max blending

- Status: Accepted (provisional, lead)
- Date: 2026-10-12
- Decided by: lead session (default for Niklas to review); approach chosen by the shield agent
- Related: [GDR-0043](../gdr/0043-shields-as-energy-bubbles.md), [GDR-0021](../gdr/0021-shields.md), [ADR-0020](0020-style-2a-renderer.md), [ADR-0021](0021-no-fixed-raster-graphics.md), `docs/client.md` "Shield bubbles"

## Context

GDR-0043 asks for a glowing bubble per shield emitter with a shimmer, hit ripples, low-charge flicker and a
shatter; the browser load test has 20 shielded ships, and the client runs on WebGL2 (no storage buffers, no
compute). The dotted rings were one mesh and one `ColorMaterial` per emitter whose alpha changed every frame: a
draw call and a re-prepared bind group per bubble. Raster art is generated at runtime only (ADR-0021). Bevy 0.20
composes shaders as WESL (WGSL plus imports and `@if`), and the 2D view bind group carries `globals.time`.

## Options considered

- **A material per bubble with per-frame uniforms**: simple; one draw call and one bind-group update per bubble
  per frame.
- **A mesh per piece in the piece frame**: one draw call per piece; ripples change the mesh every frame anyway.
- **Atlas sprites in the FX mesh** (ring, glow and ripple regions): batched, but a shimmer, a shatter and a rim
  that stays crisp at every zoom are hard with sprites.
- **A uniform array of bubbles, indexed per quad**: still one material change per frame and a fixed size limit;
  nothing gained over vertex data.
- **One world-space mesh for every bubble, the bubble state in vertex attributes, one material that never
  changes, animation from `globals.time`** (chosen): one draw call for all bubbles, no material updates, works on
  WebGL2.
- Blending: **alpha** or **additive** (overlaps become brighter lenses) or **max** (chosen: an overlap shows the
  brighter bubble, nothing doubles up, the order of the quads does not matter; WebGL2 has `MIN`/`MAX`).

## Decision

- `crates/game/src/shield.rs` and `shield.wesl` (embedded with `embedded_asset!`, importing the mesh2d bindings).
  `BubbleMaterial: Material2d` holds one uniform of style tokens. Vertex layout: position plus six `vec4`s
  (offset from the centre, radius, seed; charge, presence, shatter progress; four hits as direction, age,
  strength). Blend: `Max` with factors `One`/`One` for colour and alpha. The shader outputs the shield colour
  (encoded when the camera composites in sRGB) times its intensity.
- The mesh is rewritten in place every frame from the piece views at the render pose, like the FX meshes; when
  empty it holds one degenerate triangle and is not rewritten again until a bubble shows.
- Hit and shatter state is kept per `(ShipId, emitter part)`, outside the piece views, so a view rebuilt by the
  same shot keeps its animation.

## Consequences

- Cost per frame: one draw call for all bubbles, about 430 bytes of vertex data per bubble (40 bubbles ≈ 17 KB),
  and per covered pixel a few value-noise lookups and four hit terms; the Voronoi cracks run only while a bubble
  shatters. The quad covers the radius plus the glow (1.7× the radius while shattering). Not measured on a GPU
  yet (SwiftShader timings in the VM say nothing); the browser load test (`?load=20`) is where to measure.
- Bubbles sit on one layer above every ship (z 200, under the weapon effects), not on their ship's layer.
- With max blending a bubble never tints what is under it and does not show over anything brighter than itself:
  the faint fill disappears over hull tiles (wanted), the rim and the hits still show.
- The shader handles the `SRGB_OUTPUT` define the camera uses (ADR-0020); tonemapping in the shader and OKLab
  compositing are not handled.

12 Oct 2026 (fleet-bar agent): the bubbles are now **two** batched meshes with the same material, one per layer:
the focused vessel's bubbles at z 200 (above it), every other bubble at z 95 (above every other ship, below the
focus at z 100). The focus flying over another ship's bubble no longer appears under it; two draw calls in all,
whatever the number of bubbles. Two non-focus ships still show each other's bubbles over their hulls (one
layer for all of them). See `docs/client.md` "Shield bubbles".

Note (13 Oct 2026): the bubbles are **one** batched mesh again, drawn after every ship (z 200), and each bubble
sits between the ship layers by depth instead of by draw order. A quad's vertex z is its piece's draw layer plus
1/16 (the focus's: z 100 plus 1/16), and every lit piece's tiles are a second, opaque mesh (the opaque twin of the
atlas material, Bevy's `Opaque2d` pass) that writes depth. The bubble pipeline tests depth without writing it, so a
bubble is hidden under the tiles of every ship above its own and shows over every ship below: two non-focus ships
no longer show each other's bubbles over their hulls. Draw layers are fixed for a ship's life
(`ship_view::draw_layer`: `10 + ship id mod 80`, an eighth per piece). Cost: one bubble draw call instead of two,
one more mesh entity and draw call per lit piece in the opaque pass. Left: a bubble can still show over a higher
ship's blended parts beside its tiles (halo, glows) and over dark pieces, whose half-transparent tiles stay
blended. Not chosen: a mesh per ship (a draw call and a mesh upload per shielded ship per frame) and a few bucket
meshes at fixed z (ships in the same bucket keep the bug). See `docs/client.md` "Shield bubbles".
