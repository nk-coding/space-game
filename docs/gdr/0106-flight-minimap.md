# GDR-0106: A minimap in flight: an always-on disc bottom right on a voyage, north up round the flown ship, 1 km (B: 2.5 km, then off), only what the sensors and scans revealed, drawn by the cluster map's painter

- Status: Accepted (provisional, lead), 17 Oct 2026, for Niklas to review
  2026-10-18 (Niklas): the minimap is a button: its tooltip names the cluster map and its key, a click pins the
  cluster map open (closed by Esc, a click or the key; [GDR-0090](0090-action-map-settings-and-pause-menu.md)'s status
  line of this date).
- Date: 2026-10-17
- Decided by: lead session (defaults for Niklas's request of 17 Oct 2026 for a minimap while flying)
- Related: [GDR-0095](0095-sensors-hidden-sites-and-scans.md), [GDR-0097](0097-galaxy-map-and-voyage-flow.md), [GDR-0104](0104-scenery-rocks-filler-landmarks.md), [GDR-0090](0090-action-map-settings-and-pause-menu.md), [GDR-0050](0050-fleet-bar.md), `docs/client.md` "Voyages"

## Context

The flight camera shows a few hundred metres of a 2–3 km cluster. The edge markers point at found sites and at
enemies off screen, and the cluster map (N, GDR-0097) shows the whole cluster, but it covers the view's middle. Niklas
asked for a compact minimap that is always there while flying: the fleet, contacts, found sites, bearings, rock
fields, the cluster's edge and a range scale, without giving away what the fleet has not found.

## Options considered

- **Where**: bottom right (chosen: the target panel and the SCAN pill hold the top right, the hull block and the
  speed the left, the title and the fleet bar the top centre, the help line the bottom centre; on touch it moves up
  above the fire button); top right under the SCAN pill (its height changes with the scanner and the behavior lines
  in a test); bottom left (the touch stick, the assists list).
- **Orientation**: north up, centred on the flown ship, its heading a tick (chosen: the camera never turns, so the
  minimap reads like the view and like the cluster map, and bearings and sites keep their place while the ship
  turns); heading up (turns under the player with every A/D tap and disagrees with the view).
- **Range**: 1 km radius, B for 2.5 km and again to hide it (chosen: 1 km shows the 800 m sensor reach whole with
  room round it; 2.5 km covers a cluster from anywhere in it); one fixed range; following the camera's zoom (the
  camera's few hundred metres are on screen already).
- **What it shows**: only what the fleet knows (chosen, GDR-0095): sites found, bearings until their site is found,
  scans' sweeps, contacts within a non-dormant vessel's sensors (`sim::Voyage::sensors`, the director's own list), a
  site's ships only once the site is found and not while it sleeps (its ring stands for them), the rocks spawned so
  far (fields the fleet came near: terrain stays charted once seen); everything spawned (gives away hidden sites).
- **Drawing**: one painter for the cluster map and the minimap (`flight_hud/chart.rs`: a chart is a centre, a
  scale and a clip disc; chosen: no second copy, and the cluster map gains the rocks, the sensor rings and the
  contacts); a second widget (a copy of the cluster map's code).
- **Contacts by colour**: enemies running coral, dark or dormant vessels of another side grey, wrecks (salvage of
  at least 3 parts; smaller bits would speckle a fight) gold, the fleet teal (the flown vessel brighter, a heading
  tick).

## Decision

*(Numbers placeholders: `minimap.*` in `art/tokens.json`.)*

- On a voyage the HUD shows the minimap bottom right (28 px from the right, 46 px up; on touch 184 px up, above the
  fire button): a dark disc of 76 px radius (scaled by UI SIZE like the HUD), a thin ring, `N` at its top, a faint
  thin ring at half the range, and under it `RANGE 1.0 KM · B`.
- North up, centred on the flown ship (the camera without one). Drawn: the cluster's edge dotted, the entry, the
  rocks spawned so far as faint specks, each scan's sweep in faint violet, bearings dashed gold until their site is
  found, sites found in their marker's colour with rings, each non-dormant vessel's sensor reach as a faint dotted teal
  ring, contacts within the sensors (above), the fleet.
- **MINIMAP** (`minimap`, B by default, a mission action of the flight; no clash: B is the drydock's paint tool and
  home's BEHAVIOR, other contexts) steps it: 1 km → 2.5 km → hidden → 1 km. The step stays for the session.
- It hides while the cluster map is open, fades with the HUD (H), and is a `HudZone`: edge markers keep clear of it.
- Not in the test range or the fight viewer (no cluster).
- The cluster map draws with the same painter and so also shows the rocks, the sensor rings and the contacts.

## Consequences

- Per frame: one pass over the ships' pieces, the spawned rocks, sites, bearings and scans, a few hundred dots and
  lines in the FX queue at most; nothing allocated.
- On touch B is absent; the minimap stays at 1 km (a tap to step it can come with the touch pass).
- The sim gained a read-only accessor (`Voyage::sensors`), no rule change, no hash change.
