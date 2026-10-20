# ADR-0051: egui runs on Barlow alone (no default fonts); Bevy's `reflect_auto_register` stays

- Status: Accepted (provisional, lead)
- Date: 2026-10-21
- Decided by: lead session (default for Niklas to review), from the size cuts ADR-0049 left open
- Related: ADR-0049, ADR-0048, `docs/client.md` "Publishing the web build", "BEHAVIOR screen" (TEXT view)

## Context

ADR-0049 left two further size cuts open: egui's default fonts and Bevy's `reflect_auto_register`. Both were
measured as `web-release` builds (`trunk build --release --cargo-profile web-release`, wasm-opt -O3) on 20–21 Oct
2026. The baseline is the game's wasm at 34,755,207 bytes raw and 11,031,892 gzip (-9).

| Build | Raw | gzip |
|---|---|---|
| baseline | 34,755,207 | 11,031,892 |
| without `reflect_auto_register` | 34,761,493 (+6 KB) | 11,035,073 (+3 KB) |
| without both | 33,381,554 | 10,255,299 |
| so without egui's default fonts | **−1,379,939 (−1.32 MiB)** | **−779,774 (−0.74 MiB)** |

- **egui's default fonts** (`bevy_egui`'s `default_fonts`): Ubuntu Light, Hack, Noto Emoji and emoji-icon-font, 1.41 MB
  embedded. egui is only the BEHAVIOR screen's TEXT editor, which already drew in Barlow, with egui's fonts as the
  fallback for glyphs Barlow lacks.
- **`reflect_auto_register`**: nothing in the game uses the type registry (no scenes, no inspector, no remote, no
  entity cloning). But leaving the feature out saves nothing on wasm. bevy_reflect's default features (pulled in by
  about ten Bevy crates and bevy_egui) keep `auto_register_inventory` on, so every `#[derive(Reflect)]` still emits an
  inventory constructor. wasm-ld then wraps every export of the cdylib in a call to `__wasm_call_ctors`, so the
  constructors, and every type's registration with them, stay linked whether or not the registry is ever filled
  (twiggy: 646 `get_type_registration`/`RegisterForReflection` functions still present).

## Options considered

- **Keep both**: the largest download.
- **Drop egui's default fonts, Barlow for every egui family** (chosen): 1.32 MiB raw, 0.74 MiB gzip. What Barlow lacks
  shows as `?` in the editor: arrows (the parser accepts a typed `→` for `->`), emoji, ✓. The game's own text
  (canonical `->`, `≤`, `≥`, `·`, error messages) is all in Barlow.
- **Keep only Ubuntu Light as a fallback** (a direct dependency on `epaint_default_fonts`): 0.36 MB for arrows in user
  text that nothing in the game writes; not worth it.
- **Drop `reflect_auto_register`**: no saving (+6 KB) and gives up registry lookups for nothing.
- **Get rid of the inventory constructors** (patching bevy_reflect's default features in every Bevy crate): unmeasured,
  a fork of Bevy's manifests to keep up on every upgrade; not tried.

## Decision

- `bevy_egui` without `default_fonts`. `game::behavior_ui::text_view::install_fonts` (every screen,
  `EguiPrimaryContextPass`) gives egui Barlow Regular, already embedded for bevy_ui, as the font of the proportional,
  monospace and `barlow` families as soon as the context exists. egui has no fonts before that.
- Bevy's `reflect_auto_register` stays in the feature list.

## Consequences

- The published game's wasm is about 33.1 MiB raw, 10.3 MiB gzip.
- Any future egui UI must draw in Barlow (no monospace look) and gets `?` for arrows, emoji and other symbols.
  A typed `→` in the TEXT view parses but shows as `?`.
- Re-check `reflect_auto_register` when Bevy or wasm-bindgen change how constructors run on wasm.
