# GDR-0105: A cluster's sky: a planet, its moons, a nebula and a far galaxy, rolled from the cluster's seed, in soft vertex-colour style ("style B"), drawn only

- Status: Accepted (provisional, lead), 17 Oct 2026: the style is Niklas's (B, "soft banded", from the design study),
  the numbers are lead defaults for him to review
- Date: 2026-10-17
- Decided by: Niklas (style B; planets "too simple", clouds "stepped blobs" in the study: they must be soft and
  gradual) and lead session (numbers, layout, content shape)
- Related: [GDR-0104](0104-scenery-rocks-filler-landmarks.md), [GDR-0102](0102-grown-mechanics-and-parts-defaults.md),
  [GDR-0094](0094-galaxy-generator-defaults.md), [ADR-0020](../adr/0020-style-2a-renderer.md),
  [ADR-0021](../adr/0021-no-fixed-raster-graphics.md), `docs/client.md` "Voyages" (the sky),
  `docs/content-model.md` "The sky", `docs/results/sky-*.png`

## Context

GDR-0104 filled the space between a cluster's sites; behind it the backdrop was still the same gradient and stars in
every cluster. A design study (a canvas page, 17 Oct 2026) tried three styles of a procedural sky per cluster:
Niklas chose style B, soft vertex-colour gradients, with every layer: 13 planet kinds and 6 moon kinds, rings,
atmospheres from none to thick, a nebula of soft blobs with faint dust lanes, a logarithmic-arm galaxy, the kinds
biased by the cluster type. His feedback for the game: the planets' detail was too simple, and the clouds were
stepped blobs; they must be soft and gradual. Rough edges of the study: several moons of one kind in a system, and a
gallery the game does not need.

## Options considered

- **How a planet is drawn**: meshes whose vertex colours are worked out once on the CPU like a fragment shader (the
  surface, clouds as a smooth density, the air, the light at every vertex of an even polar mesh; thin geometry for
  what must stay crisp) (chosen: every edge a gradient, nothing stepped, vector only, one draw per body, no new
  shader); stacked translucent shapes as in the study (not chosen: the stepped clouds Niklas saw, and overdraw); a
  planet shader (not chosen: a second material path for one layer; the CPU result is built once per voyage).
- **Where the sky's kinds live**: a content kind `planet` (`planets.ron`: body, form, palettes, air, rings) and a
  `sky` block on cluster types and bands weighting them (chosen: future content adds kinds and skies; the form names
  the client's painter, the palettes its colours); kinds in code (not chosen: content in data).
- **What it is rolled from**: the cluster's own seed in the galaxy (chosen: a cluster keeps its sky at every visit);
  the voyage's seed (not chosen: a new sky each launch).
- **How far it moves**: a share of the camera's motion on screen, kept within reach of the view (chosen); true world
  parallax (not chosen: a voyage carries the camera kilometres, the planet would leave the view in seconds at the
  usual zoom).

## Decision

- **Content**: `planets.ron`, 13 planet kinds (jovian, ringed, ice giant, hot giant, toxic haze, living, garden, ocean,
  eyeball, desert, ice world, lava, cratered rock) and 6 moon kinds (grey, icy, geyser, sulfur, crystal, rubble).
  Each has `body` (`Planet`, `Moon`), `form` (one of 17 painters), `palettes` (colour sets `#rrggbb` in the order the
  form reads them, plus the air's colour; one rolled per body), `atmosphere` (`None`, `Thin`, `Moderate`, `Thick`)
  and optional `rings` (chance, colour, bright). Colours are part of a kind's art, like a part's SVG icon in the pack,
  so they sit in the content, not in `art/tokens.json`. A `sky` (on a cluster type; on a band for its content
  voyages and for its cluster types without one): planet weights (empty: any kind alike), moon weights, the nebula's
  two colours, its core colour and strength, the galaxy's strength, `backlit`. Lead defaults: the rim band (rock,
  ice, ice giants, rings, desert, eyeball), the outer reach (gardens first; living, ocean, eyeball), the border drift
  (a mix of both), the scorched deep (lava, hot giants, desert, toxic; sulfur moons), the uncharted deep (any planet,
  backlit; crystal and geyser moons). Not in the rules' fingerprint (cluster types and bands are not, and voyages
  carry no sky); the wiki has a page per kind and "The sky" on cluster type and band pages.
- **Layout** (the study's, in a 1600 × 900 frame scaled to the view's height): 60 % of planets large (0.34–0.56 of
  the height) and cut by a corner, the rest small (0.11–0.19) and whole, away from the view's middle; a large ringed
  planet three quarters the size, its rings leaning towards the view's middle so they cross it; 0–3 moons (weights
  1, 3, 3, 2), **never two of one kind**; the light from the hot star's side, from behind in a backlit sky, else from
  the view's middle side; each body keeps its distance to the left or right edge of the view; the galaxy away from
  the planet and the middle.
- **Look**: bands with wavy soft boundaries and fine ripples, turbulent streaks between them, storms with a spiral
  texture; land with rough coasts, shallows, sand, lowland and highland; patches; ice caps; clouds as one smooth
  density: soft blobs frayed by noise, wispy streaks along the latitudes, storm swirls (two logarithmic arms round a
  dense core and a small eye); fine mottling on rock, lava, desert, ice and sulfur; craters with a dark floor and a
  rim lit on the light's side; cracks, glowing rifts and vents; a garden's lights on the night side; geyser plumes;
  a crystal moon's facets; the air as a tint at the limb and a glow outside it (all round when backlit); rings as
  feathered elliptical bands, the back half under the body and in its shadow. The terminator is soft, softer with more
  air.
- **Depth** (lead defaults, feel over physics): galaxy 0, nebula 0.005, planet 0.05, moons 0.075 of the camera's
  motion on screen; the planet layer's shift stays within 0.3 of the view's height (moving further out it slows to
  nothing). The far stars move at 0.02 (0.12 before), so the planet never reads as farther than they are. Order: the
  gradient, the galaxy, the stars, the nebula, the rings' back halves, the planet, the rings' front halves, the moons,
  the hot star's glow and corona, then the landmarks and the dust.
- **Strength**: everything at 85 % (veiled towards the sky's dark) so it stays quiet behind a fight; the nebula at
  most 3 blobs over any point (2 beside a hot star, whose glow is a large translucent layer too), so no point lies
  under more than three large translucent layers.
- **Cost** (built once per voyage, static meshes, one draw per body; `sky: …` in the log): a large planet 21–33 k
  vertices (with rings), a small one 7–10 k, moons 0.5–3 k together, nebula about 2.9 k, galaxy 6.6 k; built in a few
  tens of ms in the browser.
- **Testing**: `?planet=<kind>` shows that planet kind instead of the rolled one (for looking at kinds).

## Consequences

- Every cluster looks its own and keeps its look; a new cluster type or band names its sky in content, and new kinds
  reuse a form with their own colours. A new form needs a painter in the client.
- Content voyages show their band's sky, rolled from the voyage's id.
- Open: the sky's colours do not tint the backdrop's gradient or its stars (the study's per-cluster background);
  planets do not turn and clouds do not drift; no sky outside flight (the station, the map) and no thumbnails on the
  galaxy map.
