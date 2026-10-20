# ADR-0045: The layout class follows the window; touch slop, hover and list drags live in the UI kit

- Status: Proposed
- Date: 2026-10-19
- Decided by: phones agent (proposal, phase 1 of the small-screen work)
- Related: ADR-0006, ADR-0038, GDR-0008, GDR-0009, `docs/plans/android.md` 4.1, `docs/client.md` "Phones"

## Context

Niklas wants the UI testable on phone sizes in Chrome DevTools' device mode (landscape, about 844–915 × 390–412 CSS
px, device pixel ratio 2.6–3, touch events instead of the mouse) before the Android port. `docs/plans/android.md`
4.1 already proposes a compact layout class by window size (height under 500 dp, phone layouts for 844 × 390 scaled
by min(w / 844, h / 390) clamped 0.9–1.3) and 48 dp touch targets via hit slop. Audit on 19 Oct 2026 (844 × 390,
DPR 3): no screen overflows (the 1280 × 720 layouts are fitted down to a scale of 0.54 since 13 Oct 2026), but their
type is then about 4–7 dp high, unreadable on a phone; and touch could press `Hot` nodes but had no hover, no slop,
did not count as being over a `UiBlock` panel, and could not scroll any list (the kit owned only the wheel).

## Options considered

- **Raise the floor of the 1280 × 720 fit on phones** (e.g. scale at least 0.8 and let the layouts overflow): text
  readable but controls cut off at the edges; not chosen.
- **Touch as an emulated mouse** (a kit system writing the first finger into the cursor and the left button): every
  mouse-only interaction would work at once, but Bevy then tries to move the OS cursor every frame (unsupported on
  web and Android) and the screens that already read touches (flight stick, map, BEHAVIOR sliders, fleet bar)
  would see every finger twice; not chosen.
- **Class by window in the kit, phone layouts per screen, touch basics in the kit** (chosen).

## Decision

- `kit::ui_kit::LayoutClass` (resource, `Wide` / `Compact`) follows the primary window: `Compact` under
  `COMPACT_HEIGHT` = 500 logical px (*placeholder*). `compact_scale(size)` = min(w / 844, h / 390) clamped 0.9–1.3
  (*placeholders*) is the UI scale of a phone layout. Wide windows are unchanged.
- Until a screen has its phone layout it keeps its 1280 × 720 layout fitted by `ui_scale_for` (it fits, small);
  each screen switches on `LayoutClass` when its phone layout lands (follow-up work packages).
- The kit's hit test treats the first finger down as the pointer (`UiPointer::position`, `UiPointer::finger`): hover
  shows while the finger is down, a finger on a `UiBlock` panel blocks the world, and under a finger a `Hot` node is
  hit within its box grown to `TOUCH_TARGET` = 48 dp square (the nearest such node when no box holds the finger;
  the mouse hits only the drawn box).
- A finger dragged along any `Scrollbar` area scrolls it 1:1 after `FINGER_SLOP` = 10 dp along its axis, and the
  press on the row under it then ends without a click (no fling yet).

## Consequences

- Chrome DevTools' device mode and `scripts/web-drive.mjs --dpr=3` (844 × 390) show the compact class; 1280 × 720
  and larger look and behave as before.
- Mouse-only interactions in screens (drydock painting and panning, the camera's drag, the replay's scrubber,
  `list.rs`'s chip drag, tooltips) still need touch, per screen, through `UiPointer::position` / `finger`.
- Phone layouts for each screen are follow-up work (`docs/client.md` "Phones" lists them).
- 20 Oct 2026 (mobile browser check, `docs/results/mobile-check-2026-10-20.md`): `compact_scale`'s floor went from 0.9
  to 0.75 (*placeholder*): at 0.9 the 844 × 390 layouts lost their right edge on a 667 × 375 phone and their bottom
  in a phone's browser, whose toolbars take 30 to 60 dp of the height.
- 21 Oct 2026 (Niklas): the about 7 CSS px smallest type this gives a 667 × 375 phone is accepted (no layouts of
  their own for small phones); on a phone the research screen's top bar shows only the materials the selected node
  costs and those in store (all of them wrapped over the tree and the detail).
