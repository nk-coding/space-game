# GDR-0107: Depth layers: a layer of parallax p moves p of the camera's motion and scales with the zoom to the power p, draws by its depth (nothing farther than the planet over it), stars in three layers, no distant fights, finer rocks

- Status: Accepted (provisional, lead), 17 Oct 2026, for Niklas to review
  2026-10-18: Niklas's backdrop tuning (provisional numbers, lead): the depth order by parallax is now consistent,
  galaxy 0 and nebula 0.005 < stars 0.01 / 0.03 / 0.06 (were 0.02 / 0.07 / 0.2: the near layer moved too fast) <
  planet 0.09 and moons 0.12 (were 0.05 / 0.075; its drift cap stays 0.3 of the view's height) < far junk 0.3 (the
  backdrop's far dust, was 0.35, and the debris everywhere, was 0.97, drawn at −799.5 between the dusts) < near dust 0.7
  < glints 0.85 < site debris 0.97. Thinner: far stars 36 a tile (were 70; mid 24, near 10, were 30 / 12), far dust
  2800 and near dust 700 a tile (were 3600 / 1200), motes 130 and glints 12 (were 220 / 18), debris 220 a tile (was
  320), site groups 7–12 bits (were 10–18). Smaller planets: large and corner-cut a quarter of skies (was 60 %), 0.22–0.34
  of the view's height (was 0.34–0.56), small 0.07–0.13 (was 0.11–0.19), moons 7–40 px (were 10–60), rings with the
  planet. The galaxy a soft glowing spiral: 36 wide faint sprites along each arm under 2200 finer, fainter dots (were
  1250), a core of five smooth rings. Landmarks stay at 0.03–0.04. Screenshots `docs/results/backdrop-before.png`,
  `backdrop-after.png`.
  2026-10-18, later (Niklas: the planet could not leave the view; provisional numbers, lead): the drift cap is gone,
  every piece of the sky hangs at an anchor in the cluster with true parallax like the rest of the scenery, and a sky
  has two or three planets (30 / 70 %) of different kinds, at most one large (25 %), each with its own moons (moon
  kinds still once a sky). The first is anchored 0.7 of the radius from the centre towards the entry, the others
  0.4–0.6 of it round the centre a turn's share apart, each in the view's middle when the camera is at its anchor.
  Slower, so the planets' share of the sky is in view from most of a cluster: stars 0.005 / 0.011 / 0.017 (were
  0.01 / 0.03 / 0.06), nebula 0.003 (was 0.005), planets 0.024 ± 15 % by size (bigger nearer and faster; was 0.09),
  moons a third over their planet's (was 0.12), landmarks 0.012–0.016 (were 0.03–0.04, so they stay behind the
  planets); junk and play unchanged. Sampled over 100 seeds of every sky and 200 camera places each at the starting
  zoom in a 1280 × 720 view, a planet is in view from 96 % of a 2 km cluster, 82 % at 2.5 km, 67 % at 3 km, 44 % at
  4 km (the stars' "a tad slower" became about a third: with planets near the old speeds three planets cover 13–25 %).
  2026-10-18, third (Niklas: three planets piled on each other at Corran Reach's entry, a 1 km cluster, a large one
  filling half the view; provisional numbers, lead): planets distant and small, 0.05–0.09 of the view's height and
  the one large at most 0.10–0.12 (were 0.07–0.13 and 0.22–0.34), moons 4–24 px; moons a tenth over their planet's
  parallax (were a third: they swung hundreds of px off it). Anchors are rolled after the moons and pushed farther
  out (up to 5 × the old distance) until no two planets' discs, rings and moons included, come within 24 px on
  screen for any camera within 1.15 of the radius at view heights of 720–1125 px (exact for the parallax drift; a
  zoom changes every size by `zoom^p`, 0.95–1.07, so zooming in never blows a planet up); a planet that finds no
  clear place is left out (none in 300 skies sampled at 0.6–4 km). Planets and moons draw by parallax, the nearer
  over the farther (rings just under and over their planet; the ring halves were a whole step apart). A planet is
  in view from 100 / 89 / 72 / 57 / 37 % of a 1 / 2 / 2.5 / 3 / 4 km cluster (was 96 / 82 / 67 / 44 % of 2–4 km).
  Screenshots `docs/results/sky-corran-start.png`, `sky-corran-zoom.png`, `sky-clear-a.png`, `sky-clear-b.png`.
  Three planets and their moons: 33 k vertices on average, 55 k at most (one planet with moons was 7–36 k). Home shows
  the home band's nebula and galaxy (no planets, strength 0.55). Screenshots `docs/results/sky-planets-a.png`,
  `sky-planets-b.png`, `home-sky.png`.
  2026-10-18, fourth (Niklas: three planets and their moons still piled up in the middle of Corran Reach's view;
  "the idea of multiple planets is that most of the time some is on screen"; provisional numbers, lead): the sky's
  travel was too short for its planets (a planets' layer moves radius × parallax × 18 px from the centre to the
  edge: 430 px at Corran Reach, 1 km, at 0.024, less than half a view). The planets' parallax is now set per sky
  (`sky::depth`): the most planets, up to 4, whose 1280 × 720 views fill 0.9 of that reach at a parallax of at
  most 0.04, never under 0.021 (planets 0.018–0.046 with their ± 15 %, above the stars and landmarks, far below the
  junk; zoomed all the way in a planet grows by at most 12 %, was 7 %): 1 planet at 600 m, 2 at 1 km (0.038), 2–4 at
  2–2.5 km, 4 at 3–4 km (0.021). The first planet shows in the entry's view to a side (0.16–0.26 of the width off the
  middle, level or up to 0.16 of the height towards the centre), each other is the best of 64 anchors at least
  900 px from the planets before it and clear of them by the share of 128 camera places that see exactly one planet,
  kept only if it raises it. Exactly one planet is in view from 94 / 73 / 73 / 71 / 62 % of a 1 / 2 / 2.5 / 3 / 4 km
  cluster, one or more from 95 / 78 / 78 / 80 / 64 %, three or more from 0 / 0 / 0 / 0.1 / 0 %. The night side under
  air is softer (the terminator's twilight about twice as wide, the night keeping 7 % + 10 % × the air of the
  surface's colour; airless bodies unchanged). Screenshots `docs/results/sky-spread-corran-start.png`,
  `sky-spread-corran-500m.png`, `sky-spread-3km-a.png`, `sky-spread-3km-b.png`.
  2026-10-18, fifth (Niklas: approach C for rocks, light from the sky): the finer rock outline drawn over a coarse
  collider is replaced: the sim's rocks have the drawn outline themselves (a spline through dented control points,
  in two or three convex pieces where deeply dented), and the client shades it in soft bands lit from the sky's hot
  star or first planet ([GDR-0104](0104-scenery-rocks-filler-landmarks.md)'s status line of 18 Oct 2026). Rocks stay
  at z 3, their glints and embers at 3.01 in an additive mesh; ships keep their own light.
  2026-10-18, sixth (Niklas approved the backdrop study's version 8, galaxy variant H): the galaxy is three static
  meshes built once per voyage at parallax 0, at −897, −896.99 and −896.98 (still over the gradient, under the far
  stars): a premultiplied field (a 31 × 128 polar grid sampling light and dust; an edge-on disc's 110 × 25), a small
  alpha dust mesh and an additive glow. Its kind is rolled per sky: a spiral 60 % (variant H, M74's structure as Niklas tuned it in the study's round 9:
  no hard lines and no dots; a broad field disc with arms only modestly brighter than the gaps, the arms as soft
  strands along 38 short wound segments, broad ones carrying the form and finer ones within, a few soft knots, soft
  dark dust strands; about 18 k vertices), an elliptical 12 %, an edge-on disc 16 % or an irregular 12 % (variant D,
  a soft field with splats; 3.5–6 k). The colours are rolled per galaxy within plausible ranges (core, arms, knots,
  dust), the arms nudged towards the sky's nebula. Home's sky uses it too (a spiral). Screenshots `docs/results/sky-galaxy-spiral.png`,
  `home-sky.png`.
  2026-10-18, seventh (Niklas: two sets of parallel lines at 90° through the galaxy, the same stripes on rocky
  planets, the ice worlds' crack lines look bad, the landmarks read like planets; provisional numbers, lead): the
  galaxy's noise (the study's, products of two crossed sine waves: a grid whatever its rotation) and the planets'
  (two plane waves an octave: families of parallel stripes) are now one isotropic noise for the whole sky
  (`roll::Noise`: octaves of gradient noise on a hashed lattice, each turned by its own random rotation and shifted;
  the same in every direction within 10 % by test), the galaxy's patches about as large as before, the study page
  likewise. Cracks are soft lineae (a thin core in a wide soft edge, fading in and out along the line, swelling and
  thinning with the noise; an ice world's long, gently curving and tinted reddish brown close to the ground's
  colour), no longer hard lines of one width. Landmarks move in front of the planets: parallax 0.09 (junk ring),
  0.11 (station), 0.13 (beacon), between the planets' 0.018–0.046 and the far debris's 0.3, at z −820 (over the
  planets at −887 to −884, under the far debris at −799.5; were 0.012–0.016 at −888.5), so they move clearly faster
  than the planets and, by the zoom^p rule, grow 0.84–1.27 times over the zoom's range (3–160 px/m); sizes 2.6–3.6,
  1.1–1.5 and 2.4–3.2 m of their layer (47–65, 20–27 and 43–58 px at the starting zoom, smaller than the planets'
  72–173 px discs; were 9–14, 3–4.5 and 16–24 m). The station is a structure, no longer round: a braced truss,
  modules along it, one or two pairs of solar panels, a torn docking ring at a slant at one end. The corona is
  unchanged. Screenshots `docs/results/sky-galaxy-close-before.png` / `-after.png`, `sky-rock-close-…`,
  `sky-desert-close-…`, `sky-ice-close-…` (a debugging build, galaxy and planet enlarged), `sky-station-planet.png`,
  `sky-galaxy-spiral.png`, `home-sky.png`.
  2026-10-18, eighth (Niklas: galaxies smaller, no crystal moons, a richer shipyard sky; provisional numbers, lead):
  a far galaxy's radius is rolled 35–70 px of the design frame (was 70–140: half the size on screen, every kind
  alike; `galaxy::RADIUS`). The crystal moon kind is gone (content, `PlanetForm::Crystal` and its facet painter).
  The shipyard's sky is the galaxy's `home_sky` (`galaxy.ron`, `data::HomeSky`): three nebula clouds in the rim's
  colours, each along a curve of its own in its third of the view (blobs 0.7 the size, 3–5 washes each, the colour
  pairs in turn first → second, second → core, core → first), under the same overdraw cap, at strength 0.8 (was
  0.55), and an irregular galaxy of a rolled one's size (so half the old) in a warm amber (`#ffa05a`, its clumps and knots
  from that colour), far from the shipyard's teals and blues, placed top right clear of the yard (`at: (0.9, 0.2)`). Screenshots `docs/results/home-sky-clouds.png`, `sky-galaxy-half.png`.
- Date: 2026-10-17
- Decided by: lead session (defaults for Niklas's play-test of 17 Oct 2026 of GDR-0104's scenery and GDR-0105's sky)
- Related: [GDR-0104](0104-scenery-rocks-filler-landmarks.md), [GDR-0105](0105-cluster-sky-planets-nebula-galaxy.md), [GDR-0106](0106-flight-minimap.md), `docs/client.md` "Scenery"

## Context

Niklas played the deployed scenery and sky and found: zooming felt weird (the sky kept its size on screen while
the stars, dust, debris and landmarks, all drawn in metres, scaled fully with the zoom, whatever their parallax);
things drew over the planet (landmarks, the hot star's glow and corona lay in front of it although they hang
farther off); the distant fights ("4 random dots firing at each other") had to go; the rocks were too low poly; and
the stars, slowed to 0.02 by GDR-0105, no longer gave a sense of speed.

## Options considered

- **Zoom per layer**: every layer's size on screen goes with the zoom to the power of its parallax (chosen: one rule
  from the play, 1, scaling fully, to the sky, about 0, keeping its size; each layer behaves like a picture at its
  depth, its motion and its size in the same measure); two classes, far layers fixed and near layers scaling (a
  jump between layers of nearly the same depth); everything scaling (the sky would have to, and a planet filling the
  view zoomed in is wrong for something that far).
- **Draw order**: by depth, smaller parallax farther back (chosen), with one exception: the stars are points of
  light and stay behind the sky's bodies whatever their parallax (their parallax is only for the sense of motion).
  Landmarks moved behind the planet: their parallax below its 0.05 (they are far things), the hot star's glow and its
  corona behind the planet too.
- **Stars**: three layers, far 0.02, mid 0.07 and a sparse near 0.2, all behind the planet (chosen; with the dust at
  0.35 and 0.7 the nearest layers, drawn over the sky as near dust); one layer back at 0.12 (in front of the planet's
  0.05 by parallax, so either drawn wrong or over the planet).
- **Distant fights**: removed with their content field (`scenery.fights`), its validation, wiki line, tokens and code.
- **Rocks**: the view finer than the collider (chosen: the collider stays the sim's convex hull, no hash change; the
  view rounds the hull's corners a little, resamples it to 32–96 points and roughens it by at most about 5 % of the
  radius, so hits land on what is drawn); more collider corners (a determinism change for little gain, the hull of
  more rough points is rounder anyway).

## Decision

*(Numbers placeholders.)*

- `camera::depth_scale`: a layer of parallax `p` is drawn at `camera + (anchor − camera) · p · s` and `s` times its
  size, `s = (18 px/m ÷ zoom)^(1 − p)` (18 px/m: the camera's starting zoom, `camera::REF_ZOOM`, where every layer
  looks as authored). Applied to the backdrop's dust and stars, the glints, the debris, the derelicts, the
  landmarks; the sky's pieces scale by `(zoom ÷ 18)^p` on their screen-fitted size (planet 0.05, moons 0.075, nebula
  0.005, galaxy 0). The play (rocks, ships) is `p = 1`.
- Draw order, far to near: galaxy −897, stars −893 / −892.7 / −892.4 (far, mid, near), nebula −890, the hot star's
  glow −889, its corona −888.8, landmarks −888.5, the planet's rings and the planet −887 to −885, moons −883, dust
  −800 / −799, range rings −700, glints −660, derelicts 1, debris 2, rocks 3, ships and effects above.
- Landmarks: parallax 0.03–0.04 (station 0.035, beacon 0.04, junk ring 0.03), sizes in metres of their layer at the
  reference zoom (station 9–14, beacon 3–4.5, ring 16–24: about 160–430 px).
- Stars: 70, 30 and 12 a 60 m tile (about 1080 px at the reference zoom) on the far, mid and near layers, each
  twinkling on its own; sizes 0.08–0.2, 0.12–0.24 and 0.16–0.3 m of the layer.
- No distant fights.
- Rocks: drawn outline as above; a lighter body shifted towards the light leaves a shadowed rim; 1–5 craters (one
  per 4.5 m of radius), dark bowls with an inner wall lit in the facet tone; the edge brighter facing the light.

## Consequences

- Zooming in, the far layers barely grow and move a little more against the stars; zooming out nothing in the sky
  shrinks to specks. The home screens' backdrop follows the same rule (their zoom is near 18).
- Content: `scenery.fights` is gone (a pack still giving it fails to load); landmark sizes changed meaning (metres of
  the layer at the reference zoom).
- No sim change: rocks keep their colliders, the determinism hashes stay.
