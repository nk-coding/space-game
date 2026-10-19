# GDR-0120: Flight and the fight viewer on a phone: the corner ring pauses, the voyage's salvage at the bottom, the viewer's buttons beside its timeline

- Status: Proposed
- Date: 2026-10-20
- Decided by: flight phones agent (proposal; defaults where `docs/plans/android.md` 4.2 leaves things open)
- Related: GDR-0008, GDR-0009, GDR-0040, GDR-0088, GDR-0090, GDR-0092, GDR-0106, GDR-0118, GDR-0119, ADR-0045,
  `docs/plans/android.md` 3.4, 3.7 and 4.2, `docs/client.md` "Phones"

## Context

At 844 × 390 dp (the compact class of ADR-0045) the flight HUD kept its 1280 × 720 layout at the plain UI size: the
title wrapped (`TEST RANGE · WAVE` / `1`; the fight viewer's in four lines), the touch stick and FIRE waited for a
first touch, the target panel said `CLICK OR L TO LOCK`, a voyage's left column ran down into the stick, and the fight
viewer could only be driven by keys (its timeline did not take presses). The plan (4.2) fixes the stick, FIRE, the
tap slack, the target panel's tap and a pause ring "left of the title", and leaves open where the voyage's salvage,
the minimap, the viewer's controls and the keyboard-only actions go, and how touch is detected in a browser.

## Options considered

- **Pause ring**: a second ring left of the title (plan): the title is centred between the columns, and on a phone
  the hull map ends where the title starts. **The frame's ring top left** (chosen): every screen has its ring there
  (Niklas, 18 Oct 2026, the shared frame); on a touch screen it opens the flight menu (in every scene, voyages too)
  instead of going home, and the menu has SHIPYARD or ABANDON VOYAGE…. The mouse keeps the ring going home.
- **Keyboard-only actions** (T, C, Shift+R, Z, the replay's keys): a ring menu (not built), or **buttons where they
  belong** (chosen): the flight menu's second row on touch (DRYDOCK or BEHAVIOR where T goes, WAVES OFF / ON,
  RESTART in the test range; ASSISTS OFF / ON for a ship with pilot assists), the fight viewer's buttons beside its
  timeline. Not on touch: DRIFT (it changes only the keys; the stick always flies where it points), HIDE HUD (it would
  hide the ring that brings it back), the minimap's step (always 1 km), single assist toggles (their rows sit where
  the stick's zone starts; the banner still says what Z did).
- **A voyage's SALVAGE and RECALL**: in the left column they pushed SPEED down to 350 dp, into the stick. **At the
  bottom centre** (chosen), between the stick and FIRE: SALVAGE, the number and the pips beside RECALL, what RECALL
  does under them. The **minimap** (radius 40 dp on a phone) sits over FIRE, between it and the target panel.
- **The fight viewer**: the timeline becomes a scrubber for the mouse and the finger (a thumb follows the pointer,
  the release seeks: seeking on every move would run up to 10 s of ticks a frame); on a touch screen START, −1 S,
  PLAY / PAUSE on its left and +1 S, SLOWER, FASTER on its right (the replay actions, so they act as the keys do);
  one finger pans, a tap on a ship follows it, two pinch.
- **Touch detection in the browser**: the first touch only (plan), or **the primary pointer** (chosen, with the
  first touch and `?touch=1` / `?touch=0`): `(pointer: coarse)` is what phones and Chrome DevTools' device mode
  report, so the widgets show from the start there; a touch laptop's mouse keeps the desktop HUD until its first touch.

## Decision

- **Layout** (compact class, `compact_scale`; set by the flight while it runs, SETTINGS over the flight gives it
  back): corners inside the safe area (`kit::ui_kit::SafeArea`, the plan's placeholder 32 dp at the long edges' ends,
  16 dp top and bottom, in the compact class only, until Android's insets), the hull map (84 × 44) under its number,
  values 28 dp, the title 13 dp and the subtitle 10 dp between 185 dp columns (one line in the range, two for a
  replayed fight), the banner 18 dp between the same columns. Wide windows are unchanged.
- **Touch** (plan numbers): a floating stick in the left 45 % of the window under the speed block (ring 124 dp at
  rest bottom left, faint; knob 44 dp; dead zone 6 % of the radius, full speed at the rim), FIRE 76 dp (hit 96 dp)
  bottom right, FLARE (44 dp) beside it for a ship with a flare launcher; SCAN stays under the target panel. A tap
  (under 0.3 s, under 10 dp) on a ship locks it (2 m + 24 dp of slack), on empty space unlocks; a tap on the target
  panel is LOCK TARGET (the nearest enemy, then the next). A finger that lands on a HUD button (within its 48 dp touch
  target) or the fleet bar never steers, fires, locks or pans. Two fingers zoom, and pan once their midpoint has
  moved 16 dp (the mouse's middle drag); without a pilot one finger pans too. No key hints on touch (`TAP A SHIP OR
  HERE`, `NEXT ›`; the controls overlay lists the touch controls).

All numbers are placeholders.

## Consequences

- The mouse and keys work as before; a mouse drag that starts on a HUD button no longer pans (the timeline scrubs).
- A touch button presses an action of the action map for one frame (`world::settings::ActionTaps`), so every key's
  handler serves its button unchanged.
- To try on a real phone: whether the floating stick is found from its resting ring, FIRE's 96 dp, the target panel's
  tap, and whether the bottom centre's RECALL is out of the thumbs' way.
- Open: FIRE's ring showing the guns' heat, the ability slots round FIRE as features come (overdrive, repair drones,
  autopilot), single assist toggles on touch, ENGINES / WEAPONS priority chips, the edit-HUD pencil (GDR-0008), the
  debug overlay's phone numbers (thermal headroom, atlas tier), insets from Android.
