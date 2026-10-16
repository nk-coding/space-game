---
title: "The BEHAVIOR screen"
description: "Where you write how your design thinks, as a graph or as text, with live errors, rule slots, saving, and a test flight that shows which rule decides."
---

# The BEHAVIOR screen

The BEHAVIOR screen is where you decide how the design on the drydock's grid thinks when nobody flies it. You
edit its behavior as a **graph** of rule chips or as **text**, see at once which rules your logic cores can run,
and test it in flight. How the rules run is on [How ships think](/behavior/); the text format is on
[The rule text](/behavior/rule-text).

## Opening and leaving

- In the drydock, press BEHAVIOR in the bottom bar.
- At the shipyard, go to BEHAVIOR (or press [[control:open_behavior.keys]]).

Esc goes back the way you came: to the drydock, or to the shipyard if you came from there. The ‹ DRYDOCK pill
always goes to the drydock. Leaving never changes the design and never loses your edits: the screen keeps
your draft, even across a reload.

## The layout

- **Top left:** the ring button (back to the SHIPYARD), the breadcrumb `BEHAVIOR · <DESIGN>`, the behavior's
  name with its saved state after it, and the rule slots pill, `<RULES> / <SLOTS> RULE SLOTS`: how many rules the
  behavior has, and how many rule slots your design's logic cores give.
- **Over the graph:** the tabs BEHAVIOR | PILOT ASSISTS, the design's two rule lists.
- **Top right:** GRAPH | TEXT (how the list is shown, one choice for both tabs), and TEST IN RANGE with WATCH
  and FLY ALONGSIDE under it.
- **Left:** the graph, or in the TEXT view the text editor.
- **Right:** the inspector for what you selected, then ADD NODE with its search field and the palette.
- **Bottom:** ‹ DRYDOCK, undo and redo, the status line, BEHAVIORS and SAVE.

## The GRAPH view

The graph reads like the text, top to bottom:

- **The target line** on top: the target selector as a chip, with `DEFAULT` after it while the behavior has no
  target line of its own (it then targets the nearest enemy).
- **The root**, `Priority · FIRST MATCH`, and a trunk with one dot per rule.
- **One row per rule:** violet condition chips, a connector, then a teal action chip. Actions line up in one
  column. A chip shows the node's name, its values dim after it (for example [[behavior_node:orbit]] `75% · cw`), a comparison as written
  ([[behavior_node:enemies_near]] `100m >= 3`) and `not` in violet. A rule without conditions shows a plain `always` chip. A
  script condition has a dashed outline and a gold SCRIPT tag (rest the mouse on it: a condition written as a
  small script, not built into the game). The rule's note follows, dim.
- **The slot line:** a dashed coral line above the first rule your design has no slot for,
  `<SLOTS> RULE SLOTS · THE RULES BELOW DO NOT RUN`. The rules below it are dimmed. They are kept, not deleted:
  moving a rule up is how it gets a slot.
- **The never-reached line:** under an `always` rule whose action needs no target ([[behavior_node:hold]],
  [[behavior_node:escort]]) that has rules below it, `ALWAYS MATCHES · THE RULES BELOW ARE NEVER REACHED`, and those
  rules dimmed.
- **`+ RULE · N SLOTS LEFT`** at the bottom, a dashed pill.

::: tip An `always` rule that needs a target
An `always` rule whose action needs a target steps aside while there is none, so the rules below it still run and
the editor doesn't mark them. The [[behavior:escort]] template relies on this: its last rule, the one that keeps near
the leader, runs whenever the leader has nothing locked.
:::

### Selecting

Click a chip to select it; it glows and the inspector shows it. Click a rule's `always` chip or the empty space of
its row to select the whole rule. Click the graph's empty space to select nothing: the inspector then shows the
behavior's name and summary. The wheel scrolls the graph, the inspector and the palette.

## The inspector

The inspector shows what you selected: a small kicker that says what it is (`CONDITION · VALUE`,
`ACTION · MANEUVER`, `TARGET · SELECTOR`, `CONDITION · SCRIPT`), its name in its colour, its summary, then its
controls. A script condition also shows its script in gold.

- **Numbers:** a slider with a −/+ ring on each side. The steps are round numbers that grow with the value. A
  distance has an `M` / `% REACH` switch: with your design's weapon reach known, switching keeps the distance the
  same and only changes how it is written. A share of the reach also shows the metres it comes to
  (`≈ 100 M`). A slider drag is one undo step.
- **Choices** are pills; the chosen one is teal.
- **DEFAULT** marks a value the rule leaves out (the node's default applies). RESET writes the default back.
- **A condition:** the NOT switch (TRUE WHEN IT DOES NOT HOLD), for a value the COMPARE pills (`<`, `<=`, `>`,
  `>=`) and THAN with its number, its own values, then REPLACE and REMOVE, then its rule's block.
- **An action:** its values, then its rule's block. A different action from ADD NODE replaces it.
- **The target line:** the SELECTOR pills, AIM AT (the aim point), USE THE DEFAULT TARGET while the behavior has
  a target line, and the NOTE ON THE TARGET LINE.
- **A rule's block:** `RULE 2 OF 4` (with `· NO SLOT` or `· NEVER REACHED` where that applies), UP, DOWN,
  DUPLICATE (a copy right below), DELETE, and the NOTE field. DELETE is off for the last rule: a behavior needs
  at least one.

## ADD NODE and the palette

The palette lists the whole vocabulary: conditions in violet, actions in teal, script conditions in gold with
`· SCRIPT`, target selectors in white. Press [[control:search.keys]] to jump to the search field and type part of a
name. The line over the search field says where your next pick goes: `ADD NODE · TO RULE 3`,
`ADD NODE · A SELECTOR SETS THE TARGET`, or `REPLACE HULL · PICK A CONDITION`.

What a click in the palette does depends on the selection:

| Selected | A condition | An action | A target selector |
|---|---|---|---|
| a rule, or one of its chips | is added to that rule | replaces the rule's action | sets the target line |
| a condition with REPLACE armed | takes that condition's place | replaces the rule's action | sets the target line |
| the target line | shows it in the inspector | shows it in the inspector | sets the target line |
| nothing | shows it in the inspector | shows it in the inspector | shows it in the inspector |

A node shown in the inspector this way displays its default values, read-only, and how to add it.

## Editing rules

- **Add a rule:** `+ RULE` adds `always ->` the first action of the palette. If your last rule is an `always`
  fallback, the new rule goes just above it, so the fallback stays last. The new rule is selected: pick
  conditions for it from the palette.
- **Remove:** [[control:delete.keys]] removes the selected condition, deletes the selected rule (also when its
  action is selected) or puts the target back to the default. REMOVE, DELETE and USE THE DEFAULT TARGET do the
  same.
- **Reorder:** drag a rule's row up or down; a teal line shows where it lands. Or use UP and DOWN. The order
  is the priority: the first rule that matches decides.
- **Notes:** click the NOTE field, type, and press Enter (or click elsewhere).
- **Undo and redo:** [[control:undo.keys]] and [[control:redo.keys]], or the two rings after ‹ DRYDOCK. The
  screen keeps the last [[sim:undo_steps]] steps.

Every edit keeps the behavior valid. An edit that would break it (one condition too many, a line too long, a value out
of range) is refused, and the status line says why in coral for a few seconds.

## The TEXT view

TEXT shows the behavior as [rule text](/behavior/rule-text), one rule per line, with dim line numbers in a gutter.
Names are coloured as in the graph. Type freely; copy and paste work as usual. While the text has the keyboard,
undo and redo are the text's own.

Every change is checked at once:

- Each error is printed in coral after its line, with the column underlined. The status line says
  `2 ERRORS · FIRST ON LINE 4, COLUMN 6`.
- A valid text becomes the behavior at once, as one undo step, and the graph follows. The status line says
  `VALID · 4 RULES`.
- Rules beyond your slots carry `no rule slot: this rule does not run`, rules below an `always` rule
  `never reached: an always rule above matches first` (an `always` rule whose action needs no target).

While the text has errors, the GRAPH view shows the last valid rules, read-only, and the status line says so; an
edit there is refused with "fix the text first". Leaving the TEXT view tidies a valid text into the canonical form.

## The status line

When nothing else is going on, the status line tells you how the rules stand, in this order:

- errors in the text (see above);
- `NO LOGIC CORE · THIS DESIGN CANNOT RUN RULES`;
- `2 RULES WITHOUT A SLOT · LOGIC CORES GIVE RULE SLOTS`;
- `1 RULE IS NEVER REACHED · AN ALWAYS RULE ABOVE MATCHES FIRST`;
- else `THE FIRST RULE THAT MATCHES DECIDES · GUNS FIRE AT WILL`.

A message from saving, opening or testing takes its place until you change the draft.

## Saving and the BEHAVIORS list

The **name** top left is the behavior's name: click it to rename (Enter renames, Esc keeps the old name). After
it, the saved state:

| State | Meaning |
|---|---|
| `TEMPLATE · SAVE MAKES THIS DESIGN'S OWN COPY` | a behavior of the game, flown by this design |
| `TEMPLATE · SAVE MAKES A COPY` | a behavior of the game the design doesn't fly |
| `SAVED` | one of yours, as saved |
| `UNSAVED` | edits not saved yet |
| `NOT SAVED YET` | new, never saved |

`· THE DESIGN FLIES <NAME>` follows when the behavior on the screen is not the one the design flies.

**SAVE** ([[control:save.keys]], also while you type in the TEXT view):

- a **template** (the game's own behaviors) becomes **your copy** under a new name ("Skirmisher copy", or the
  name you gave it). If the design flew the template, the design flies your copy from now on;
- a behavior of yours is updated. Renaming keeps it linked to every design that flies it;
- a text with errors is saved as you typed it, so you can fix it later. Until then, ships that should fly it
  fly their faction's default instead.

Unsaved edits are never lost: the draft is kept while you look at the drydock or test, and comes back after a
reload. If something would replace a draft with unsaved edits (opening another behavior), the screen saves it
first and says so.

**BEHAVIORS** opens the list: YOUR BEHAVIORS (most recently saved first, tagged `THIS DESIGN`, `IN THE EDITOR`,
`UNSAVED`) and the TEMPLATES. For the picked row:

- OPEN edits it (the design keeps flying its own);
- USE FOR THIS DESIGN makes the design fly it and opens it;
- DUPLICATE saves a copy, RENAME renames it, DELETE asks first (there is no undo);
- NEW BEHAVIOR starts a fresh one with one rule.

Esc or CLOSE closes the list.

## TEST IN RANGE

TEST IN RANGE (or [[control:test_flight.keys]]) flies the behavior on the screen against raider waves, as it is
now, saved or not. Pick the mode first:

- **WATCH:** your design flies on its own on this behavior. Nothing is piloted; you watch.
- **FLY ALONGSIDE:** you fly the design as usual, and a second copy flies beside you as your wingman on this
  behavior, with you as its leader, so a [[behavior_node:locked]] target follows your lock.

The test is refused, with the reason in the status line, while the text has errors or the design can't fly.

In flight, a panel under the target panel shows the behavior live:

- its title, `BEHAVIOR · <NAME>`, and which rule decides: `RULE 2 DECIDES`, or `NO RULE MATCHES · IT HOLDS`
  (`STARTING` before the first think);
- every rule as one line of the graph's chips ([[behavior_node:shield_down]] and [[behavior_node:target_on_me]] › [[behavior_node:orbit]] `75% · cw`), the deciding
  rule in a teal pill, rules without a slot dimmed;
- under each rule above the deciding one, in grey, **why it doesn't match**: `hull < 30% is false (hull is 96%)`,
  `target_in_range(100%): no target`, `no target for approach`, or a script that failed and counts as false.

[[control:test_return.keys]] ends the test and brings you back to the screen with your draft as it was. Losing
the tested ship (flying alongside: every ship of yours) ends it too. [[control:reset_scene.keys]] starts it over, [[control:waves.keys]] stops and starts
the waves, and [[control:hide_hud.keys]] hides the HUD with the panel.

## The PILOT ASSISTS tab

PILOT ASSISTS edits the design's second rule list in the same GRAPH and TEXT views, inspector and palette.
Assists have no target line: the graph's TARGET row reads `YOUR LOCK`, as they act on the ship you lock, and the
palette has no target selectors. Assists need no rule slots, so there is no slot line; the root reads
`Assists · EACH CHANNEL`, the pill
`3 ASSISTS · WHILE YOU FLY IT`, `+ ASSIST` adds one, and the list may be empty. Assists are saved with the
design, not on their own: SAVE in the drydock keeps them. What assists do is on
[Pilot assists](/behavior/pilot-assists).

## Keys

| Key | Does |
|---|---|
| Esc | back the way you came (first it cancels REPLACE; in the TEXT view it first leaves the text) |
| [[control:test_flight.keys]] | TEST IN RANGE, in the mode picked under it |
| [[control:save.keys]] | SAVE |
| [[control:search.keys]] | jump to the palette's search field |
| [[control:delete.keys]] | remove the selected condition, rule or target line |
| [[control:undo.keys]] | undo |
| [[control:redo.keys]] | redo |
| [[control:debug_overlay.keys]] | the debug overlay |

The keys work while no text field and no TEXT view has the keyboard and the BEHAVIORS list is closed; SAVE also
works while you type in the TEXT view.
