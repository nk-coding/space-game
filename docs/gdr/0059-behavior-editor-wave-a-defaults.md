# GDR-0059: Behavior editor wave A defaults: editing rules, storage and tests, the screen

- Status: Accepted (provisional, lead); flying the faction default with a banner confirmed by Niklas in [GDR-0079](0079-review-of-lead-defaults-14-oct.md). 2026-10-16: `SGD1` (and `SGD2`)
  codes no longer import; only the current share format is read.
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the wave-A behavior agents
- Related: [GDR-0057](0057-behavior-editor-v1.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md)–[GDR-0037](0037-behavior-vocabulary-as-content.md), [ADR-0031](../adr/0031-player-storage-and-design-sharing.md), `docs/behavior-format.md` "Editing model", `docs/drydock.md` "Behaviors", `docs/client.md` "Behavior screen"

## Context

GDR-0057 fixed the behavior editor's scope. Wave A built the pure editing model (`data::behavior_edit`), the
plumbing (storage, designs naming behaviors, share codes, TEST IN RANGE modes) and the screen's foundation. Each
needed choices nobody had decided; they are collected here for review after trying the editor.

## Decision

Editing model:
- A new value condition starts as `< example` (`hull < 30%`); new nodes leave optional values out (the content
  default applies); replacing a node keeps the values that still fit.
- The last rule can't be deleted (a behavior needs one); new conditions go at the end of their rule.
- The default target stays implicit (`nearest`) until a selector, aim point or note is set.

Storage and tests:
- Player behaviors are saved as content-style behaviors under `behaviors/<id>`; ids never equal a content
  behavior's ("Skirmisher" → `skirmisher_2`); a stored behavior with errors still loads as text but never flies.
- A design naming a behavior that can't fly (errors, deleted) flies its faction's default, with a banner.
- Share codes became `SGD2` (a design's custom behavior travels along; `SGD1` codes still import); a `.ron`
  export carries the behavior as a commented block after the ship.
- WATCH ends like a test flight when the watched ship is lost; test modes leave out `?fleet=` wingmen; unsaved
  text is tested under a temporary id; `?fleet=` wingmen follow the player's lock.

Screen:
- The BEHAVIOR button sits in the drydock's bottom bar, left of BLUEPRINTS; Esc or a "‹ DRYDOCK" pill goes back.
- Graph chips use content names (`Target in range 120%`), the text view the ids (`target_in_range(120%)`); the
  target line chip is neutral white.
- The text view is an egui multi-line editor styled to the UI (bevy_ui has no text input); errors show after
  their line.

## Consequences

- All of these are easy to change after Niklas's first session with the editor.
