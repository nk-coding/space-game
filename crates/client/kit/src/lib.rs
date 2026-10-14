//! The client's toolkit (ADR-0038): what every screen draws and lays out with. Knows nothing about ships or the
//! sim; `world` and `game` build on it.
//!
//! - `atlas`, `style`, `meshbuf`: the texture atlas rasterised at runtime (and its shader `atlas.wesl`) + style
//!   tokens, mesh building.
//! - `ui_kit`, `ui_icons`: bevy_ui widgets with hand-made hit testing, the UI's fonts, screen transitions and
//!   scrollbars; vector icons in UI nodes (parts and whole designs through `ui_icons::IconShapes`, which `world`
//!   implements).
//! - `clipboard`: copy / paste of UI text, per platform.
//! - `params`: the launch parameters and the render switches they set.
//! - `view`: the camera's zoom and place as the toolkit reads them (`ViewScale`, written by `world`'s camera).
//! - `rng`: a small random generator for decoration.

pub mod atlas;
pub mod clipboard;
pub mod meshbuf;
pub mod params;
pub mod rng;
pub mod style;
pub mod ui_icons;
pub mod ui_kit;
pub mod view;
