# GDR-0077: Abandoning a voyage asks in a panel instead of M twice

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (interactively); keys, placement and words by the agent that built it
- Supersedes: [GDR-0072](0072-shipyard-parts-voyage-flight-defaults.md), only its rule "M pressed twice within 3 s abandons a voyage"; that abandoning counts as lost stays, and the rest of GDR-0072 stands
- Related: [GDR-0062](0062-first-playable-loop.md), [GDR-0065](0065-phase-7-screens-and-escort-action.md), [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), `docs/client.md` "Voyages"

## Context

A ship gone dark next to an armed enemy can neither recall nor be lost otherwise, so a voyage needs a way out
(GDR-0072). The lead default was M pressed twice within 3 s, announced by a banner (`ABANDON THE VOYAGE? · M
AGAIN`): a hidden double key, a timer to beat, and a key letter in the text, which the UI otherwise avoids (labels
carry no key letters; tooltips name the keys).

## Options considered

- **M twice within 3 s** (the GDR-0072 lead default): no UI to build; but easy to miss, easy to trigger by
  accident, and it says less about what is lost.
- **M opens a confirm panel** (chosen): the question, what is lost and two buttons; the keys still work.

## Decision

Niklas, 14 Oct 2026: in a voyage, M opens an **ABANDON VOYAGE?** panel instead of arming a second M.

- The panel says what is lost (`THE HAUL IS LOST · 5 PARTS STAY BEHIND`, every hold's items) and that the voyage
  counts as lost while the ships re-form at home. Its buttons: **ABANDON VOYAGE** (coral) and **KEEP FLYING**.
- Keys (agent's choice, after the debrief and the voyage map, where Enter does the panel's action and Esc goes
  back): Enter confirms, Esc or M again close the panel. The labels carry no key letters; the buttons' tooltips
  name the keys (`ENTER`, `ESC / M`). An Esc that closes the panel keeps the target lock.
- Confirming ends the voyage as before: `VOYAGE ABANDONED`, the 3 s countdown, then lost (`Phase::Lost {
  abandoned: true }`, `VoyageEnd::Lost`).
- **The sim keeps running while the panel is open** (agent's choice): the game is a real-time sim that will be
  played in co-op, where one player's question can't stop the others' world; the panel says so (`THE FLIGHT GOES
  ON WHILE YOU DECIDE`). The player can keep flying and firing with the keys; P still pauses on its own.
- It sits centred low over the help line, so the ship at the view's centre stays in sight, and slides up from the
  bottom and back like the other HUD parts (`ui_kit::ScreenFx`). Clicks on it never reach the world (no shots, no
  lock change). It closes by itself when the voyage ends another way (every vessel lost).

## Consequences

- `voyage.rs` keeps whether the panel is open (`VoyageRun::asking`) instead of the time of the first M; the
  3 s window (`ABANDON_SECONDS`) is gone.
- Touch still has no way to abandon (no M key, no home ring in voyages); a touch button that opens the same panel
  is open for later.
