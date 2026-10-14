# GDR-0060: Behavior editor wave B defaults: editing gestures, saving and linking, the live panel

- Status: Accepted (provisional, lead); flying the faction default with a banner confirmed by Niklas in [GDR-0079](0079-review-of-lead-defaults-14-oct.md)
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the wave-B behavior agents
- Related: [GDR-0057](0057-behavior-editor-v1.md), [GDR-0059](0059-behavior-editor-wave-a-defaults.md), `docs/client.md` "Behavior screen", `docs/drydock.md` "Behaviors"

## Context

Wave B wired the behavior editor: editing in the graph and the inspector, saving and the BEHAVIORS list, TEST IN
RANGE and a live rule panel in flight. The choices below were made while building it.

## Decision

Editing:
- `+ RULE` adds `always ->` the palette's first action above a trailing `always` fallback (else at the bottom) and
  selects it.
- With a rule or one of its nodes selected, a palette condition is added to that rule, a palette action replaces
  its action, a target selector sets the target line; replacing a condition needs REPLACE armed first.
- Delete on a selected action deletes its whole rule; the last rule can't be deleted.
- Distances switch between metres and a share of the design's weapon reach (its longest clear gun); value steps
  are round numbers that grow with the value; a slider drag is one undo step.
- Rules are reordered by dragging their row (or UP / DOWN).

Saving and linking:
- SAVE (or Ctrl+S) turns a template into the player's own copy; the design is linked to the copy only if it flew
  that template. Saving a player behavior updates it.
- Unsaved edits stay in memory and in a working slot (they survive a reload) and are saved before anything
  replaces them, as the drydock does with designs.
- In the BEHAVIORS list, OPEN only opens a behavior for editing; USE FOR THIS DESIGN links it.
- A behavior with errors can be saved and linked as work in progress; flights use the faction default until it is
  fixed.

TEST IN RANGE: the draft is tested as it is (saved or not); a live panel in flight lists the rules, highlights the
deciding one and says why the higher rules don't match.

## Consequences

- All of these are easy to change after Niklas's first session with the editor.
