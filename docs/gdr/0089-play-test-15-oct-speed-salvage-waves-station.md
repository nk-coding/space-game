# GDR-0089: Play-test of 15 Oct: the velocity follows the push at top speed, salvage settles, test-range waves bring bigger enemies, the station comes alive

- Status: Accepted (provisional, lead)
- Date: 2026-10-15
- Decided by: lead session, from Niklas's play-test notes and answers (the speed model: "you find a way to make this feel better"; the waves: "fill by size", chosen interactively)
- Related: [GDR-0041](0041-engine-rated-speed.md), [GDR-0055](0055-slower-crisper-turning.md), [GDR-0064](0064-lock-feed-forward-and-ai-flight-fixes.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0079](0079-review-of-lead-defaults-14-oct.md), [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), [GDR-0071](0071-shipyard-home-long-u.md), `docs/sim.md` "Top speed and the cruise flame" and "Salvage", `docs/client.md` "Scenes"

## Context

Niklas played on 15 Oct 2026 and noted:

- **Top speed felt erratic.** At the top speed, a push in another direction sometimes took the ship past it and
  sometimes barely did anything. He wanted pushing a new way to reach the top speed that way, with the speed the
  old way falling away.
  - Cause: the whole-speed fade was eased by `min(|v|, 4 × speed along the row)` (GDR-0064). At the rating, a push
    75.5° or less off the motion got no thrust, and one 78.5° or more off got full thrust. Between the two lay a
    3° window.
  - A ship that kept turning while pushing in the full band could gain speed tick by tick. Nothing bounded it,
    and no test covered it.
- **Cargo was hard to reach.**
  - There is no drag in the sim, and a wreck kept its ship's velocity.
  - A raider killed at full speed drifted on at about the player's own top speed.
  - The collect pass takes nothing faster than 8 m/s relative to the collector.
- **Test-range waves were swarms.** Every wave was only raiders, so a big ship fought up to eight small ones.
- **The station felt static.** Some of its joints also looked wrong:
  - the gate's pylons ended in empty space next to the ring;
  - the target lane overlapped its tower;
  - the bridges to the moorings ended above and below the mind;
  - FLEET's hover marked only the near two moorings.

## Options considered

- **Speed: keep the fade and widen the window** (a smaller `FADE_ALONG`). This moves the edge but keeps it. A ship
  that turns while it pushes still gains speed without a bound. Not taken.
- **Speed: a hard clamp on |v|.** It would also clip blasts and rams, and the clamp would decide the top speed
  instead of the engines. Not taken.
- **Speed: a smooth fade by angle plus a governor on the ship's own thrust** (chosen).
  - The fade uses `speed × cos²θ` (θ is the push's angle to the motion). Straight along the motion this is the
    old whole-speed fade, so the knee and the cruise flame are unchanged. Off the motion the fade lets go
    smoothly, with full thrust at the rating from about 27° off.
  - The governor cuts only the part of the ship's own force that would raise the speed past its top speed. What
    is left turns the velocity towards the push.
- **Salvage: cut the velocity once, when a ship dies.** It would be a visible jerk. Not taken.
- **Salvage: settle over time** (chosen). Salvage faster than a slow drift loses speed steadily until it drifts
  slowly.
- **Waves: grow with the wave number.** Not chosen (Niklas).
- **Waves: fill by size** (chosen by Niklas). The same budget is spent on the biggest enemies first.

## Decision

- **Speed** (supersedes GDR-0064's `FADE_ALONG` and GDR-0055's "a diagonal is no faster than the slowest group
  pushing that way"):
  - A row pushing with the motion fades by `rated_falloff(|v| cos²θ, rated)`.
  - **The governor** (`ship::govern`) caps the force a piece's own actuators put on it so that, in that tick, it
    never takes the speed past the piece's top speed. The top speed is the highest rating among the groups that
    can push now.
  - If the ship is already faster than its top speed (a blast), the governor holds that speed instead of
    raising it.
  - Collisions and blasts are never cut. Braking passes unchanged.
  - Result: at the top speed, pushing a new way swings the velocity onto it at the top speed.
  - A ship may now reach its top speed diagonally, where before it was capped by the slower group. The cap is now
    the ship's top speed.
- **Salvage settles** (`SalvageTuning::settle_speed` 3 m/s, `settle_decel` 4 m/s²; placeholders):
  - Pieces that are not a vessel, and loose items, slow down at 4 m/s² until they drift at 3 m/s or less.
  - A wreck from 20 m/s is slow after about 4 s and about 50 m.
- **Waves** (`scene::wave_mix`, placeholder rule):
  - The budget is half the flown design's build points.
  - It goes to the biggest `enemy`-tagged ships first, each as often as it fits whole, then one more of the
    smallest if at least half of it is left.
  - At least 1 ship, at most 8.
  - Examples: the starter meets a hunter; the Halvex Runner meets two hunters and a raider; a 300-point design
    meets two wardens.
- **The station** (home):
  - Beacons breathe and flash.
  - The gate's ring turns while it is awake, and a light runs up its approach.
  - The mind's spokes turn around a beating core.
  - Research's dishes sweep and its orbit turns while research runs.
  - Two shuttles ply from the far moorings to research and to the salvage bay.
  - Outside flight the dust drifts slowly. Far stars twinkle in three groups.
  - The mind gained cognition banks, heat sinks, coolant runs and a collar into the base.
  - Research gained a dome, server racks and mounts for its dishes.
  - Joints:
    - the bridges bend into the mind's corner chamfers;
    - the pylons are trusses from the arm tips to the gate's pods;
    - the lane hangs off its tower on a strut and is shorter.
  - FLEET's place covers all four moorings.
- **Range rings** (the dotted rings around the flown ship) keep their dots at least 2.6 px across and 11 px apart
  at any zoom, so they stay visible zoomed out.

## Consequences

- The sim's state hashes change (flight and salvage). The determinism check must pass again.
- The speed tests changed. Measured on 15 Oct 2026:
  - a test (`at_top_speed_the_velocity_swings_onto_the_push_and_never_passes_it`) has a 20 m/s ship at its
    rating push 30°, 60°, 89° and 120° off the motion. Each time the velocity ends on the nose (within 4°) at
    20.00 m/s and never passes it;
  - a locked Runner pushing across its motion at the top speed peaks at 22.00 m/s, where it used to reach
    22.7;
  - the RCS box in drift with W and E ends at 20 m/s with 12.7 m/s sideways, up from 7.9;
  - the locked diagonal stays at 14 m/s, because `Kin::top_along` still asks for the conservative goal.
- The balance numbers in `docs/results/` predate this change. Wrecks that settle make salvage easier on voyages.
- The station's moving parts are small meshes moved by transforms. No mesh is rebuilt while they move.
