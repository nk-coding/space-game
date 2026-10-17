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
