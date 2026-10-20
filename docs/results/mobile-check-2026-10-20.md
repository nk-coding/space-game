# Mobile browser check, 20 Oct 2026

A follow-up of [ADR-0048](../adr/0048-the-web-build-is-a-player-platform.md): the web build is published on GitHub
Pages, so phones will open it. Every screen and the main flows were checked in headless Chromium's phone emulation
(`scripts/web-drive.mjs --dpr=N` / `--touch`, `?dev=1`, the everyday `web-dev` build), each screenshot looked at.

## Sizes

| Tag | CSS size | DPR | What it stands for |
|---|---|---|---|
| s667 | 667 × 375 | 2 | a small phone (iPhone SE / 8) in landscape |
| p844 | 844 × 390 | 3 | a common phone, the size the phone layouts are made for |
| c844 | 844 × 340 | 3 | the same phone in its browser, toolbars taking about 50 dp of the height |
| b932 | 932 × 430 | 3 | a big phone |
| port | 390 × 844 | 3 | a phone in portrait |
| tab | 1180 × 820 | 2 | a tablet with touch (the wide layouts) |

## What was checked

- Screens: home, drydock, BEHAVIOR, research, FLEET, WORKSHOP, the LOG, the voyage map, SETTINGS (CONTROLS, DISPLAY,
  SAVES), EVOLUTION (empty, NEW RUN's setup scrolled by a finger), the test range (stick, FIRE, the flight menu), a
  voyage (HUD, minimap, RECALL, the menu), the debrief after a recall.
- At s667 and c844 after the fix below every screen once again; flight at s667, c844, b932 and the tablet; the
  desktop at 1280 × 720, 1600 × 900 and 2560 × 1440 (`scripts/ui-check.sh`, home and SETTINGS).
- The page: the viewport meta, the canvas size (equal to the window: 844 × 390 CSS px, visual viewport scale 1), the
  computed styles for touch (below). No panic or page error in any run.

## Fixed

- **The phone layouts did not fit small phones or a phone's browser** (commit `bf81abe`): `compact_scale` stopped
  at 0.9, so at 667 × 375 the 844 × 390 layouts were laid out 741 dp wide and lost about 100 dp at the right (the
  research tree started off the left edge with its materials over the detail, BEHAVIOR's `4 / 12 RULE SLOTS` ran
  into GRAPH, the drydock's bottom bar cut FLIGHT-READY, home's note over TEST RANGE), and in a phone's browser
  (844 × 340: the browser's toolbars stay, since the page never scrolls) the bottom 12 dp. The floor is 0.75 now
  (*placeholder*, ADR-0045 note): everything fits at 667 × 375 (0.79) and 844 × 340 (0.87). Before and after:
  `mobile-667-research-before-2026-10-20.png`, `mobile-667-research-after-2026-10-20.png`. Wide windows are
  unchanged (the class is chosen by height under 500 dp).
- **The page under a finger** (`index.html`, landed in the loading screen's commit `d88dbfb`, which took the whole
  file): no `viewport-fit=cover` (the game drew under a notch and the home indicator; the browser now keeps the page
  clear of them, the screens lay out to the window's edges and nothing reads `env(safe-area-inset-*)`);
  `touch-action: none` and `overscroll-behavior: none` on the page (no double-tap zoom, pull to refresh or bounce on
  the overlays either, not only on the canvas), no text selection, no long-press callout, no tap flash; iOS Safari's
  `gesturestart`/`gesturechange` prevented (it ignores `user-scalable=no` for a pinch). The canvas already had
  `touch-action: none`, `oncontextmenu` off and winit's `prevent_default_event_handling`.

## Seen fine

Flight on every size (the stick, FIRE, the ring and its menu, a voyage's SALVAGE, RECALL and minimap; at 1180 × 820
the wide HUD with the touch widgets), the debrief, SETTINGS' three tabs including SAVES (rows and buttons whole at
844 and 667), FLEET, WORKSHOP, the LOG, EVOLUTION's setup and its finger scroll, the map, BEHAVIOR, the drydock at
844 × 340. The tablet's wide layouts all fit; their 7 to 8 px labels are as small as on a 1280 × 720 desktop.
Portrait shows the existing "rotate your device to landscape" cover ([GDR-0008](../gdr/0008-ui-ship-mind-interface.md):
landscape only).

## Needs a decision (for Niklas)

1. **Type on small phones.** At 667 × 375 the phone layouts now fit, but at 0.79 their 9 px floor
   (`CompactText(9)`) is about 7 CSS px (14 device px at DPR 2): readable, small (`mobile-667-evolution-setup-2026-10-20.png`).
   Options: (a) accept it for phones under about 700 dp wide (recommended: few phones are that small, and 844 ×
   390 stays at 9 px); (b) a larger floor below 0.85 (lines will wrap and some screens overflow again); (c) layouts
   of their own for 667 × 375.
2. **Research's materials on a phone.** Every material any research costs is shown in the top bar (18 in the base
   pack now, 12 on 19 Oct), and on a phone they wrap into four or five rows over the tier heads and the detail's top
   (`mobile-844-research-materials-2026-10-20.png`; the dev mode's 1000000s make it worse, a career gets there with
   its store). Options: (a) on a phone only the materials the selected node costs plus those in store, ALSO IN STORE
   style (recommended); (b) one row that scrolls sideways; (c) a MATERIALS chip that opens the list.
3. **Tablets in portrait.** The rotate cover shows for every coarse pointer in portrait, so an iPad held upright
   gets it too (`mobile-tablet-portrait-2026-10-20.png`); the wide layouts would fit at about 0.64 scale. Options:
   (a) keep landscape only everywhere (recommended: GDR-0008, nothing to build); (b) the cover only under about 600
   dp wide, tablets get the 1280 × 720 layouts scaled down.

## Still open (no decision needed)

- Home's career note at 667 × 375 overlaps TEST RANGE's line (the station is fitted to the whole window, the note
  sits over its corner; `mobile-667-home-note-2026-10-20.png`). Fine at 844 and up.
- At 667 × 375 SETTINGS, FLEET and research leave 40 to 60 dp empty at the bottom: the layouts keep 390 dp of
  height and the window is 474 dp at that scale. Their bottom lines could hang from the window's bottom.
- The voyage map's labels overlap where clusters crowd (`BRASA FOGNANDEEP`), on every size; not phone-specific.
- Desktop, seen by the way: SETTINGS › CONTROLS' BLINK description takes three lines and touches EMP BURST's row,
  and EMP BURST has no description.
- `SafeArea` stays the plan's placeholders (no web insets are read; with `viewport-fit=cover` gone, none are needed).
- Not checkable headless: real browser toolbars (simulated by the 844 × 340 size), iOS Safari itself, the pinch.
- **Headless note**: at DPR 3 SwiftShader draws about 1.7 frames a second, so a `tap` on a control with a tooltip
  (home's gear ring, SET COURSE) lasts more than the 0.35 s tooltip hold between two frames and shows the tip
  instead of clicking. Drive flows with `--touch` at DPR 1 (about 7 frames a second, the same compact layout) and
  take the pictures at DPR 3.
