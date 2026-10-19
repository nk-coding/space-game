# GDR-0121: Tooltips and flings by touch: a long press shows a control's name, a lifted list flings on

- Status: Proposed
- Date: 2026-10-20
- Decided by: phones sweep agent (proposal; Niklas was away, defaults where `docs/plans/android.md` 4.1 leaves things open)
- Related: ADR-0045, GDR-0118, GDR-0119, GDR-0120, `docs/plans/android.md` 4.1, `docs/client.md` "Phones"

## Context

The kit's tooltips (`Tip`: a control's name and key) showed after a mouse hover only, so on a phone the icon-only
controls (rings, the drydock's tool strip, undo and redo) had no name. A finger's drag scrolled a list 1:1 but it
stopped dead on lifting. Both are kit-wide (`kit::ui_kit` `tip.rs`, `scrollbar.rs`) and must not fight the holds the
screens already use: BEHAVIOR's rule rows (held 350 ms within 10 dp, GDR-0119), the drydock's grid (120 ms) and its
palette rows dragged sideways (GDR-0118), the flight's stick and FIRE (GDR-0120).

## Options considered

- **Tooltip by touch**: (a) on a tap, then the control does nothing until a second tap: every button costs two
  taps; (b) on a long press that still clicks on release: holding to read a name would press DELETE; (c) **on a
  long press that never clicks** (chosen, Android's long-press tooltips): held 0.35 s without moving 10 dp (the
  thresholds of BEHAVIOR's hold) on a node with a `Tip`, the name shows above the finger until it lifts or moves on,
  and the press then ends without a click. No key line on touch. A press a scroll took first shows none.
- **Meeting the screens' holds**: a priority rule between the kit and the screens, or **keeping them apart**
  (chosen): every hold the screens use is on a node without a tip, so the two never meet; a screen that adds a hold
  to a tipped control has to drop the tip there.
- **Fling**: physics-style friction (distance follows speed squared), or **the wheel's glide** (chosen, the plan's
  suggestion): `Glide::fling` runs the same ease-out over 0.9 s from the finger's speed over its last 0.1 s (the
  curve starts at three times its mean speed, so the list leaves the finger as fast as it went); none under
  120 dp/s, at most 5000 dp/s; it ends at the content's end, never past it. A finger landing on a moving list stops
  it, and that press is no click.

## Decision

The chosen options above. All numbers are *placeholders* (0.35 s, 10 dp, 0.9 s, 120 and 5000 dp/s).

## Consequences

- Every tipped control names itself on a phone without any screen's code; icon-only buttons are usable.
- A long press on a button is safe: it never presses it.
- Every kit list (`Scrollbar` areas, pooled `ScrollRows` lists too) flings; the screens' own pans (the drydock's
  grid, BEHAVIOR's sideways graph) do not yet.
- Checked: unit tests (`tip.rs`, `scrollbar.rs`) and a long press headless; a flick cannot be sampled headless
  (about 8 frames a second), so the fling's feel waits for a real phone.
