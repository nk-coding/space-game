---
title: "The rule text"
description: "How a behavior is written as text, one line per rule, for the TEXT view and for content files, with every limit, the errors and the grammar."
---

# The rule text

Every behavior can be read and written as plain text, one line per rule. The TEXT view of the
[BEHAVIOR screen](/behavior/editor) shows exactly this text, content files store exactly these lines, and the
graph is drawn from it. This page is the reference for players who type their behaviors and for modders who
write them in content files. How the rules run is on [How ships think](/behavior/).

```behavior
target weakest(engines) # cripple the slowest first
when hull < 30% -> flee # run when badly hurt
when target_in_range(120%) and cycle(9s) >= 5s -> orbit(60%)
always -> approach(85%)
```

## Lines

| Line | Meaning |
|---|---|
| `target <selector>` | Optional, and only as the first line: who the rules are about. Without it, the behavior targets the nearest enemy. |
| `when <condition> and <condition> -> <action>` | A rule: every condition must hold for the action to run. |
| `always -> <action>` | A rule without conditions. It matches every time, so it is usually the last one. |
| `... # note` | A note at the end of a line. It belongs to that rule (or to the target line) and survives the GRAPH view. |

Blank lines are ignored. There is no `end`, no `or`, no brackets around conditions and no nesting. For "this
**or** that", write two rules with the same action:

```behavior
when hull < 30% -> flee # badly hurt
when enemies_near(100m) >= 3 -> flee # or badly outnumbered
always -> approach(70%)
```

## Conditions and comparisons

A condition is a node name, with its values in brackets if it takes any.

- A **flag** stands alone: `shield_down`, `under_attack`, `target_in_range(60m)`.
- A **value** is compared with `<`, `<=`, `>` or `>=` and a number: `hull < 30%`, `enemies_near(150m) >= 3`,
  `cycle(9s) >= 5s`. The number is in the value's own unit and is never below 0.
- `not` goes in front: `not target_in_range(100%)`, `not hull < 50%`.
- `and` joins conditions. A rule has at most [[sim:max_conditions]] of them.

Which nodes are flags and which are values, with the unit each value measures in, is listed on
[How ships think](/behavior/#conditions) and on each node's page.

## Values and units

Values go in brackets after the node name, separated by commas, in the order the node lists them:
[[behavior_node:orbit.signature]] takes a radius, then a direction. A node takes at most
[[sim:max_values]] values.

| Kind | Write | Notes |
|---|---|---|
| Distance | `45m` or `45` | metres |
| Distance | `60%` | a share of **your weapon reach**, the range of your longest working gun; between 0% and [[sim:max_reach_percent]] |
| Percentage | `30%` | always with `%`: `hull < 30` is an error, so that 30% and 0.3 can never be mixed up |
| Time | `5s` or `5` | seconds |
| Count | `3` | whole numbers, no unit |
| Choice | `cw` | one of the words the node lists, e.g. the aim points `centre`, `engines`, `weapons`, `power`, `core` |

Numbers use a decimal point and a leading digit: `0.5`, not `.5` or `0,5`. The unit may follow after a
space (`30 %`, `45 m`). Each value has a range of its own (an orbit's radius, a cycle's period); the node's
page and the inspector show it.

A share of the reach makes one behavior fit every ship: `approach(67%)` stops at two thirds of whatever the
ship's longest gun reaches. A ship without a working gun counts a reach of [[sim:default_reach]].

### Defaults

Values you leave out take the node's default. `orbit` alone circles at radius
[[behavior_node:orbit.default.radius]], direction [[behavior_node:orbit.default.dir]]; `orbit(80%)` changes the
radius and keeps the default direction. Values are positional, so to set the direction you write the radius
too: `orbit(60%, cw)`. Brackets with nothing in them are allowed and mean the same as none: `flee()` is
`flee`.

A behavior without a target line targets [[behavior_node:nearest]], aiming at the ship's centre.

## Typing on a phone

- Names and keywords are not case-sensitive: `When Hull < 30% -> Flee` works and is shown in lower case.
- `if` works for `when`, `→` for `->`, and `≤` and `≥` for `<=` and `>=`.
- Spaces are free: `orbit( 45 , cw )` is `orbit(45, cw)`.

The game always prints the **canonical** form: lower case, `when` and `->`, one space around
`->`, the comparisons and `#`, and a comma and a space between values.
Reading that print again gives the same behavior, and leaving the TEXT view tidies your text into it.

## Notes

A note starts with `#` and runs to the end of the line. It belongs to the rule, so it stays with the rule when
you move it in the graph, and it is shown dim after the rule there. A note is one line of text: no tabs or other
control characters. A line that is only a note is an error, because the graph would have nowhere to keep it.

## Limits

| What | At most |
|---|---|
| Rules in a behavior | [[sim:max_rules]] (and at least one) |
| Conditions in a rule | [[sim:max_conditions]] |
| Values of a node | [[sim:max_values]] |
| Characters in a line | [[sim:max_line_chars]], both as typed and as printed |

The real limit in play is much lower: a ship runs only as many rules as its logic cores give
[rule slots](/behavior/#rule-slots).

A line must fit both **as typed and as printed**, because the canonical print may be longer than what you typed:
`always->flee#run` prints as `always -> flee # run`. A line that grows past the limit when printed is refused
as "too long once written out"; shorten the note or drop a condition.

## Errors

Every error names the line and the column and says what was expected. The TEXT view prints each error after its
line, underlines the column, and the status line counts them. A text with an error changes nothing: the
editor keeps the last valid rules (the GRAPH view shows them, read-only) until you fix it.

Mistakes in how a line is written:

```text
when hull < 30% flee
when hull = 30% -> flee
when hull < 30%, shield_down -> flee
when target_in_range(60m) or under_attack -> face
when orbit(45 -> flee
when cycle(9s) >= .5s -> kite
when target_in_range(5km) -> face
flee
# run away
always and hull < 30% -> flee
```

```text
line 1, column 17: expected `and` or `->`, found `flee`
line 2, column 11: compare with <, <=, > or >= (there is no `=`); the arrow is `->`
line 3, column 16: join conditions with `and`
line 4, column 27: there is no `or`: write a second rule with the same action
line 5, column 15: missing `)`
line 6, column 19: write numbers with a leading digit, e.g. 0.5
line 7, column 23: unknown unit `km`; units are m (metres), s (seconds) and %
line 8, column 1: a rule starts with `when` or `always` (or `target` for the target line), not `flee`
line 9, column 1: a note goes at the end of a rule (`... -> flee # note`), so it survives the graph view
line 10, column 8: `always` stands alone; drop it or the other conditions
```

Once every line reads, the words and values are checked against the vocabulary:

```text
target nearest(engine)
when sheild_down -> orbit(80%, ccw)
when hull < 30 -> flee
when hull -> flee
when shield_down < 50% -> kite
when enemies_near(150m) >= 2.5 -> kite(120%)
always -> orbit(60%, sideways)
when flee -> hold
always -> aproach(70%)
when target_in_range(60m) -> face(100%)
always -> brake
```

```text
line 1, column 16: `engine` is not an option; choose one of: centre, engines, weapons, power, core
line 2, column 6: unknown condition `sheild_down`; did you mean `shield_down`?
line 3, column 13: write a percentage with `%`, e.g. 30%
line 4, column 6: `hull` is a value (a percentage): compare it, e.g. `hull < 30%`
line 5, column 20: `shield_down` is true or false; it cannot be compared
line 6, column 28: expected a whole number, found 2.5
line 7, column 22: `sideways` is not an option; choose one of: auto, ccw, cw
line 8, column 6: `flee` is an action, not a condition
line 9, column 11: unknown action `aproach`; did you mean `approach`?
line 10, column 35: `face` takes no values
line 11, column 11: `brake` is for pilot assists only
```

Other messages you may meet:

- a value out of its range: "Range must be at most …" or "… must be at least …", and for a share of the reach
  "… must be between 0% and [[sim:max_reach_percent]] of the weapon reach";
- a unit that doesn't fit: "a time cannot be in `m`; write seconds (5s)";
- a word where a number belongs: "expected a distance, found the word `far`" for `target_in_range(far)`;
- a keyword used as a name: "expected a condition, found the keyword `not`" (the keywords are `when`, `if`,
  `always`, `and`, `or`, `not` and `target`);
- `!`, `&` or `|`: "write `not` instead of `!`", "join conditions with `and`", "there is no `or`: …";
- a second target line, or one below the rules: "only one `target` line is allowed", "the `target` line comes
  before the rules";
- no rule at all: "a behavior needs at least one rule";
- too many rules or conditions: "at most … rules", "at most … conditions per rule";
- a line over the limit: "line too long", or "too long once written out" for its print.

## Grammar

```text
program     = { line newline }                   blank lines are ignored
line        = target-line | rule
target-line = "target" call [ note ]             at most one, before the rules
rule        = ( "when" | "if" ) cond { "and" cond } arrow call [ note ]
            | "always" arrow call [ note ]
cond        = [ "not" ] call [ cmp number ]
call        = name [ "(" [ value { "," value } ] ")" ]
value       = number | name
number      = [ "-" ] digits [ "." digits ] [ unit ]      unit: "m" | "s" | "%", spaces allowed before it
cmp         = "<" | "<=" | ">" | ">=" | "≤" | "≥"
arrow       = "->" | "→"
note        = "#" text to the end of the line           no tabs or other control characters
name        = letter { letter | digit | "_" }          case-insensitive
```

Spaces and tabs between tokens are free. The keywords cannot be node or option names.

## In content files

For modders: a behavior in a content pack's `behaviors/` folder holds exactly these lines. The `target` field is
the target line without its keyword, and `rules` holds one string per rule, notes included. No conversion is
involved, so the wiki, the TEXT view and the file always say the same.

```ron
(
    id: "picket",
    name: "Picket",
    summary: "Wait until something attacks, meet it nose first, then circle it.",
    tags: ["attack"],
    target: "attacker",
    rules: [
        "when closing_in(3s) and not target_in_range(100%) -> face # meet it nose first",
        "when target_in_range(100%) -> orbit(70%)",
        "always -> hold # nobody aims at me: wait",
    ],
)
```

The fields are `id`, `name`, `summary`, and optionally `description`, `lore`, `tags` and `target` (default
`nearest`); `rules` is required. Unknown fields are errors. In a file the target may also be written with its
keyword (`target: "target attacker"`). An empty string in `rules`, or a target line among the rules, is an error,
and so is a list longer than [[sim:max_rules]] rules. The content check reports every problem with the field
(`rules[2]`) and the column.

A faction names the default behavior of its ships (`behavior: "picket"` in the faction file), and a ship's
blueprint can name its own the same way; it wins over the faction's. The behavior a ship flies must fit the rule
slots of the logic cores on its main piece: the content check refuses one with more rules than that. Behaviors
you save in the game are stored in exactly this format, so a saved behavior can be dropped into a pack's
`behaviors/` folder as it is.

### Script conditions for modders

A condition node with a `script` field is a custom condition written in Rhai. It lives with the other
conditions in the pack's `behavior_nodes/` folder and is tagged RHAI in the editor:

```ron
(
    id: "outranged",
    name: "Outranged",
    summary: "The target's guns reach farther than yours by more than the given share.",
    tags: ["target", "rhai"],
    role: Condition,
    params: [(id: "margin", name: "Margin", type: Percent, default: "10%", min: 0, max: 100)],
    script: "target.exists && target.reach > me.reach * (1.0 + margin)",
)
```

The rules for a scripted node:

- It is a **flag** (no `value`): the script ends in `true` or `false`. Its id must not be a built-in node's.
- Its parameters are numbers (distance, percentage, time or count; no choices yet), and none may be called `me`
  or `target`. The script reads them by id in base units: metres (a `%` distance already turned into metres),
  shares from 0 to 1, seconds, counts.
- It sees `me` and `target`, read-only. `target.exists` is false without a target, and every number of it is
  then 0.

| Name | Meaning |
|---|---|
| `.hull` `.shield` `.heat` `.power` | shares from 0 to 1, as the conditions of the same name |
| `.x` `.y` `.vx` `.vy` `.speed` `.angle` | position (m), velocity (m/s), nose angle (radians, counter-clockwise, 0 = up) |
| `.reach` | weapon reach (m); `me.reach` is the reach `%` distances use, [[sim:default_reach]] without a working gun; `target.reach` is 0 without one |
| `target.distance` `target.bearing` `target.closing` `target.aims_at_me` | distance (m), angle off your nose (radians, left positive), closing speed (m/s, negative while it opens), whether it aims at you |
| `me.enemies` | how many active enemy ships there are |
| functions | `abs` `min` `max` `clamp` `floor` `ceil` `sqrt` `sin` `cos` `atan2` `exp` `pi` `to_float` `to_int` |

Scripts run sandboxed: no printing, no time, no files, no `import`, no `eval`, and no `**` (powers are not
exact on every platform; multiply instead). Each run has a budget of [[sim:script_max_ops]] operations, and
small limits on depth, strings, arrays and variables. Scripts are compiled when the content loads, so a syntax
error or a misspelt variable name is a content error. At run time, a script that runs out of its budget, fails
or returns anything but `true` or `false` counts as **false**, and the test panel says why.
