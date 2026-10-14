# GDR-0087: Voyage transitions: the fleet visibly warps out and in, the camera eases in during the charge, every end dips to dark

- Status: Accepted (provisional, lead)
- Date: 2026-10-14
- Decided by: Niklas asked for it ("the recall etc. could get a better animation or so", 14 Oct 2026); the look,
  the timings and the camera behaviour are lead defaults for him to review
- Related: [GDR-0062](0062-first-playable-loop.md), [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), [GDR-0077](0077-abandon-voyage-asks-in-a-panel.md), [GDR-0079](0079-review-of-lead-defaults-14-oct.md), `docs/client.md` "Voyages", "Screen transitions"

## Context

A voyage's transitions were text: RECALL showed `WARPING HOME · n` for the 3 s warp-out (GDR-0062) and then cut to
home; a voyage started with the fleet simply there; losing every vessel or abandoning the voyage showed the lost
banner and cut to home. The sim's rules and timing (3 s, `voyage::HOME_TICKS`) stay as they are; this record is
about what the player sees during them, and the parts of it that touch play: the camera and the HUD.

## Options considered

- **Camera during the warp-out**: keep following as always; hold still while the fleet jumps away; or ease in a
  little while the fleet charges and hold through the jump (chosen: it frames the build-up; the fleet keeps flying
  and the view loses only 12 %).
- **The fleet leaving**: fade out in place; or stretch along the heading and dart ahead out of sight with a flash and
  a ring where it left (chosen: reads as a jump, and the arrival can be its reverse).
- **The emergency recall** (no working gun, GDR-0074): the same look; or the same shape, unstable: amber and coral,
  flickering, shaking (chosen: the palette's warning colours, so it reads as the rough way out it is).
- **Hand-over to home**: a hard cut (as before); a white flash (rejected: a full-screen white-out); or a short dip
  to the backdrop's darkest colour that the next screen comes up out of, the HUD closing with its screen transition
  (chosen).

## Decision

- **Warp-out** (3 s): every vessel charges for 2.25 s (a glow under it, a ring that tightens, a dashed ring spinning
  up, streaks drawn in), then stretches along its heading and darts ahead out of sight in 0.3 s, a flash and a ring
  where it left; the escorts jump a moment before the flagship. The camera eases in by 12 % over the charge and holds.
  The banner and its countdown stay. The emergency recall charges in amber and coral, flickering, its jump shaking,
  and its banner says `EMERGENCY WARP · n` in gold.
- **Arrival**: a voyage's first moments are the reverse: the world comes up out of the dark while the HUD slides in,
  the fleet streaks in from behind and settles with a flash and a ring, the camera eases back out from 12 % in.
- **Every end** (recalled, lost, abandoned; and a lost test flight going back to the drydock): over the last 0.36 s
  the HUD closes with its screen transition and the window dips to dark; the next screen comes up out of it (0.5 s).
  After a recall a ring of light spreads from home's gate. A loss dims the world behind the lost banner (40 %) first.
- Client visuals only: nothing reaches the sim; the player keeps control during the warp as before. Timings and looks
  are placeholders (`warp_fx.rs`'s tuning block).

## Consequences

- `warp_fx.rs` holds all of it (pure, tested curves); `CameraRig::warp_zoom` is the camera's hook; the HUD's world
  markers (lock ring, enemy and site arrows) now fade with the HUD instead of vanishing (also with H).
- The fleet is out of sight for the last 0.45 s of the warp-out while the sim still flies it; losing the last vessel
  in that moment still loses the voyage (GDR-0062), the dip then comes with the lost banner.
- Open: sound (none yet), a reduced-motion setting (none in the game yet), the look of a warp between clusters when
  the galaxy has them (GDR-0078).
