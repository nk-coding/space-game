# GDR-0079: Niklas's review of the lead defaults (14 Oct 2026): most confirmed; faster collecting, loose items last the voyage, speed zoom

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (fourteen questions, interactively)
- Supersedes: GDR-0066 and GDR-0070 in part (the collecting speed; the loose items' lifetime; the camera adds a
  speed zoom)
- Related: [GDR-0053](0053-drydock-v1-lead-defaults.md), [GDR-0056](0056-drydock-wave-2-defaults.md), [GDR-0058](0058-turning-v2-defaults.md), [GDR-0059](0059-behavior-editor-wave-a-defaults.md), [GDR-0060](0060-behavior-editor-wave-b-defaults.md), [GDR-0066](0066-phase-7-wave-1-defaults.md), [GDR-0070](0070-phase-7-wave-2-defaults.md), [GDR-0072](0072-shipyard-parts-voyage-flight-defaults.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0076](0076-spilled-cargo-becomes-fragments.md)

## Context

Eight records held defaults the lead had set for Niklas to review ("Accepted (provisional, lead)"). Parts were
already settled by later records or by Niklas on 13 and 14 Oct; some he wants to judge in play (the starter's
turn, the Warden's clear rate, the raider's turn). The rest went to him as fourteen multiple-choice questions,
the current default first.

## Decision

**Changed:**
- **Collecting** works up to **8 m/s** relative to the salvage (was 4 m/s); the reach stays 5 m hull to hull
  (GDR-0066/0070). You can scoop while drifting past slowly. "Fly slowly over a wreck" (GDR-0063) stands.
- **Loose items** (core fragments, spilled cargo) **last until the voyage ends** (was 60 s; GDR-0066, GDR-0076).
- **Camera in flight**: the lock framing stays (zoom out to keep the locked target on screen; a hand zoom turns
  it off until the lock ends), and the camera **also zooms out with speed** (Niklas: "there should also be some
  auto zoom based on speed"). Its numbers are a lead default (placeholders): up to 1.4× wider at the vessel's
  top speed, eased over about 0.8 s.

**Confirmed as they are:**
- RCS nozzles push 200 kN, so pods are also low-speed boosters (GDR-0058).
- The A/D aim nudge while locked keeps its soft lag (not fed forward) (GDR-0058/0070).
- A/D turning never uses differential thrust (GDR-0058).
- A core gives 1 programmable matter whether stripped intact or destroyed (GDR-0066).
- An enemy counts as armed while its controlling piece has an alive gun and an alive reactor (GDR-0070).
- The 3 s recall warp-out is committed and the fleet stays vulnerable during it (GDR-0072).
- Drydock frames Frigate 24×24, Cruiser 40×40, Capital 64×64 (GDR-0053).
- A design whose behavior has errors flies its faction's default behavior, with a banner (GDR-0059/0060).
- Collecting takes the best value per cargo unit first (GDR-0066).
- The right mouse button erases in every drydock tool (GDR-0056).
- The cell under a machine takes the neighbouring armour tone (GDR-0072).

**Left for play:** the starter turning faster than the Runner (GDR-0066), the Warden at about 68 % clears
(GDR-0073/0074), the raider's 0.57 s turn (GDR-0058).

## Consequences

- `sim::SalvageTuning`: `max_speed` 8 m/s; loose items no longer expire within a voyage. Voyage hashes change.
- The flight camera gets the speed zoom (`docs/client.md`).
- GDR-0053, 0056, 0058, 0059, 0060, 0066, 0070 and 0072 note the review in their status lines; their other
  defaults (the trivia not asked about) stay provisional until Niklas says otherwise.
