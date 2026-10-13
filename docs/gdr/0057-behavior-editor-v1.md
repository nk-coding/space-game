# GDR-0057: Behavior editor v1: its own screen from the drydock; behaviors are saved on their own and designs pick one; test by watching or flying alongside; graph and text views

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively)
- Related: [GDR-0008](0008-ui-ship-mind-interface.md) (behavior editor look), [GDR-0015](0015-layered-behavior.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md), [GDR-0035](0035-rule-slots-per-core.md), [GDR-0036](0036-behavior-text-format.md), [GDR-0037](0037-behavior-vocabulary-as-content.md), [GDR-0051](0051-drydock-v1.md), [ADR-0031](../adr/0031-player-storage-and-design-sharing.md), `docs/behavior-format.md`, `docs/concept/boards/ui-mockups/phone-behavior-editor.png`

## Context

With Drydock v1 done, Niklas chose the behavior editor as the next phase (over salvage, balance and the home
menu). Behaviors already exist as content (the text format, the vocabulary, rule slots, the sim's evaluation;
GDR-0034 to GDR-0037), but players can't write or change them. GDR-0008 sketches the editor: a priority list of
rules, violet conditions → teal actions, Rhai nodes tagged, limited rule slots per logic core, a graph/text
toggle and "test in range".

## Options considered

- Where: **its own screen, opened from the drydock** (chosen; later also from the home ring menu), or a panel
  inside the drydock.
- Ownership: **behaviors are their own saved documents and a design picks one** (chosen), or each design embeds
  its rules.
- Test in range: watch the design fight on the behavior, fly alongside it as your wingman, or **both** (chosen).
- Views: **graph and text with a toggle** (chosen), graph first, or text first.

## Decision

- A BEHAVIOR screen in the mockup's look, opened from the drydock for the design on the grid: the rule list as a
  graph (the target line, rules as violet condition pills → a teal action pill, Rhai nodes tagged, the rule-slot
  line from the design's logic cores), an inspector for the selected node, a palette to add nodes, and a TEXT view
  of the same rules with live errors. An edit in one view shows in the other.
- Behaviors are saved like designs (the player storage layer); a design names the behavior its ship runs when
  nobody pilots it; content behaviors are templates to copy. A design's share code carries its custom behavior.
- TEST IN RANGE has two modes: **watch** (the design flies unpiloted on the behavior against raider waves, the
  camera follows it and the active rule is highlighted live) and **fly alongside** (you fly your ship, the tested
  design joins as a wingman running the behavior). T returns to the editor.

## Consequences

- Rule slots stay a drydock trade-off: rules below the design's slots are shown inactive.
- Wingmen (`?fleet=`), escorts and test flights fly the behavior their design names.
- The home ring menu (GDR-0008) will open the same screen later.
