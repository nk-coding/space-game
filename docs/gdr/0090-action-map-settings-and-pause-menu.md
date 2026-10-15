# GDR-0090: The action map, the SETTINGS layer (controls and display), the flight menu, assist toggle slots

- Status: Accepted
- Date: 2026-10-15
- Decided by: Niklas (Esc, pausing, where settings opens, its scope, toggle slots: interactively); lead session (the
  action list, the matching rules, the layout; defaults for Niklas to review)
- Related: [GDR-0088](0088-pilot-assists-weapon-groups-and-key-bindings.md) (this is its first step),
  [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md), [GDR-0086](0086-letting-go-coasts.md),
  `docs/client.md` "Controls", `docs/content-model.md` "Controls"

## Context

GDR-0088 decided that every key moves into an action map, with defaults in data and a settings screen to rebind
them. It left four things open:

- where settings opens;
- how Esc works in flight, which so far unlocks the target, while there is no pause menu at all;
- what else the screen holds;
- how one assist rule gets its own in-mission toggle key.

Niklas also asked whether keys belong to the assists instead. They don't, because the two are separate layers:

- **Actions and their inputs** are player-wide. They live in settings.
- **Assists** are rules per design. They live on the BEHAVIOR screen.
- **Toggle slots** connect the two.

## Options considered

- **Esc in flight**:
  - **unlock, else the menu** (chosen);
  - always the menu, with unlock on its own key;
  - no menu.
- **The menu pauses**:
  - **in the test range only** (chosen; voyages never pause);
  - always;
  - never.
- **Settings at the shipyard**:
  - **a gear ring top right plus a key, a layer over the yard like FLEET** (chosen);
  - a building on the station.
- **Scope**:
  - keys only;
  - **keys and display** (chosen by Niklas).
- **An assist rule's key**:
  - **toggle slots** (chosen): settings holds ASSIST TOGGLE 1–4; a rule takes a slot and shows its key;
  - a key named in the rule, saved with the design;
  - no per-rule keys.

## Decision

- **Actions** (`data::input::Action`, one list in code; names, summaries and default inputs in the content pack's
  `controls.ron`):
  - Each action has up to two inputs. An input is a key or a mouse button, with Ctrl, Shift or Alt.
  - Each action belongs to a section on the settings screen and to the screens where it acts (its contexts).
  - Two actions that share a screen cannot share an input. Content validation checks the defaults, and
    rebinding moves a taken input over.
- **Matching** (lead):
  - An input without Ctrl or Alt never fires while Ctrl or Alt is held, so browser shortcuts and the editors'
    Ctrl combinations stay apart.
  - Shift is ignored unless the same key is bound with Shift on a screen that shares a context. So W still
    thrusts with Shift held, while the drydock's R and Shift+R stay apart.
- **What stays fixed**:
  - Esc as back or close, and Enter as confirm, everywhere.
  - Text fields and the clipboard.
  - The left mouse as pointer: click, lock, drag.
  - Middle-drag panning and the wheel.
  - Touch.
- **Flight**:
  - Esc unlocks the target while one is locked. Otherwise it opens the flight menu: RESUME, SETTINGS, SHIPYARD,
    and in a voyage ABANDON VOYAGE…, which opens the existing panel.
  - The menu pauses the test range, not a voyage.
  - The debug toggles move from 1/2/3 to F5/F6/F7, so 1–3 are free for the fire groups (GDR-0088 step 2).
- **SETTINGS** is a layer over home (the gear ring top right, key O) and over flight (from the flight menu). It
  has two tabs:
  - **CONTROLS**:
    - every action by section, with two input chips each;
    - click a chip, then press a key or mouse button, or Esc to cancel;
    - CLEAR, and RESET TO DEFAULTS.
  - **DISPLAY**: GLOW, BLOOM, and UI SIZE (100 %, 90 %, 80 % of the size that fits the window).
  - Everything saves on this device: `settings/controls` (only the changed actions) and `settings/display`.
    URL parameters still override the display settings for one run.
- **Assist toggle slots** come with the assists (GDR-0088 step 3): actions ASSIST TOGGLE 1–4 (default F1–F4) and
  ALL ASSISTS.
- **Labels**: tooltips and the flight help line show the bound inputs, not fixed letters.

## Consequences

- Every hard-coded key outside the fixed list moves to the action map. The keyboard tests press the default
  inputs.
- A mod can rebind defaults by overriding a `controls.ron` entry. The wiki can list the controls.
- Gamepads add an input kind later, without changing the actions.
