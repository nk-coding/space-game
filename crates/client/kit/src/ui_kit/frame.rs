//! The frame every screen shares (Niklas, 18 Oct 2026: "the back button sits at different positions on different
//! screens"): one set of numbers (`frame.*`, `panel.*` and `hud.*` in `art/tokens.json`) for the way back, the
//! margins and the top bar, and the helpers that place them.
//!
//! * **Top left**, at the window's corner on every screen (not the corner of a screen's 1280 × 720 stage, which
//!   moves with the window's aspect): the back ring with the menu glyph ([`spawn_head`]), the breadcrumb over the
//!   title beside it; a screen may add to the row (the EVOLVING chip, the rule slots). The shipyard has no way back:
//!   its words start at the same corner ([`Frame::corner`]).
//! * **Top right**, at the same margin: the screen's pills ([`top_pill`], [`spawn_close`]) centred on the ring's
//!   height ([`Frame::top_right`]).
//! * **Plates over the view** ([`panel_node`], [`panel_colors`], [`panel_title`]): the flight menu, the controls
//!   overlay, ABANDON VOYAGE?.
//! * **The flight HUD's type** ([`Frame::hud`]): one head, value and sub size for every group.

use std::sync::OnceLock;

use bevy::prelude::*;

use super::fonts::HudFonts;
use super::widgets::{Pill, ring_node, spawn_pill};
use super::{Tip, Tone, label, prose, tracked};
use crate::style::{UiPalette, ui_color};
use crate::ui_icons::{Glyph, UiIcon};

/// The frame's numbers (logical px at UI scale 1).
#[derive(Clone, Debug)]
pub struct Frame {
    /// The margin from the window's left and right edges.
    pub edge: f32,
    /// The margin from its top (and bottom) edge.
    pub top: f32,
    /// The back ring's diameter and its glyph's size.
    pub ring: f32,
    pub ring_glyph: f32,
    /// Between the ring, the heading and what follows them.
    pub gap: f32,
    /// The breadcrumb's and the title's text sizes, and the gap between them.
    pub crumb: f32,
    pub title: f32,
    pub line_gap: f32,
    /// The top bar's pills: height, text size, the gap between them.
    pub pill: f32,
    pub pill_text: f32,
    pub pill_gap: f32,
    pub panel: PanelLook,
    pub hud: HudType,
}

/// A plate over the view (`panel.*`).
#[derive(Clone, Debug)]
pub struct PanelLook {
    pub fill: f32,
    pub border: f32,
    pub pad_x: f32,
    pub pad_y: f32,
    pub gap: f32,
    pub title: f32,
}

/// The flight HUD's type (`hud.*`).
#[derive(Clone, Debug)]
pub struct HudType {
    pub head: f32,
    pub value: f32,
    pub value_minor: f32,
    pub sub: f32,
    pub title: f32,
    pub subtitle: f32,
    pub group_gap: f32,
}

/// The frame, read from the style tokens once.
pub fn frame() -> &'static Frame {
    static FRAME: OnceLock<Frame> = OnceLock::new();
    FRAME.get_or_init(|| {
        let t = art::tokens();
        let n = |path: &str| t.f32(path).unwrap_or_else(|| panic!("style token `{path}` missing in art/tokens.json"));
        Frame {
            edge: n("frame.edge"),
            top: n("frame.top"),
            ring: n("frame.ring"),
            ring_glyph: n("frame.ring_glyph"),
            gap: n("frame.gap"),
            crumb: n("frame.crumb"),
            title: n("frame.title"),
            line_gap: n("frame.line_gap"),
            pill: n("frame.pill"),
            pill_text: n("frame.pill_text"),
            pill_gap: n("frame.pill_gap"),
            panel: PanelLook {
                fill: n("panel.fill"),
                border: n("panel.border"),
                pad_x: n("panel.pad_x"),
                pad_y: n("panel.pad_y"),
                gap: n("panel.gap"),
                title: n("panel.title"),
            },
            hud: HudType {
                head: n("hud.head"),
                value: n("hud.value"),
                value_minor: n("hud.value_minor"),
                sub: n("hud.sub"),
                title: n("hud.title"),
                subtitle: n("hud.subtitle"),
                group_gap: n("hud.group_gap"),
            },
        }
    })
}

impl Frame {
    /// An absolute node at the window's top left corner (inside the margins).
    pub fn corner(&self) -> Node {
        Node { position_type: PositionType::Absolute, left: px(self.edge), top: px(self.top), ..default() }
    }

    /// The top left's row: the back ring, the heading, what the screen adds, centred on the ring's height.
    pub fn top_left(&self) -> Node {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(self.gap), height: px(self.ring), ..self.corner() }
    }

    /// The top right's row: the screen's pills, centred on the ring's height.
    pub fn top_right(&self) -> Node {
        Node {
            position_type: PositionType::Absolute,
            right: px(self.edge),
            top: px(self.top),
            min_height: px(self.ring),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(self.pill_gap),
            ..default()
        }
    }

    /// The breadcrumb over the title, centred on the ring's height and as tall as the ring, so the row (and the ring
    /// in it) sits at the same place on every screen whatever the title holds (a name field is a little taller).
    pub fn heading(&self) -> Node {
        Node { flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: px(self.line_gap), height: px(self.ring), ..default() }
    }
}

/// The back ring (the menu glyph) with `button` (its action, its tip) on it.
pub fn spawn_back(parent: &mut ChildSpawnerCommands, ui: &UiPalette, button: impl Bundle) -> Entity {
    let f = frame();
    parent
        .spawn((ring_node(f.ring), button))
        .with_child((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(f.ring_glyph), height: px(f.ring_glyph), ..default() }))
        .id()
}

/// The breadcrumb's text (`SHIPYARD  ›  WORKSHOP`).
pub fn crumb(fonts: &HudFonts, text: &str, ui: &UiPalette) -> impl Bundle + use<> {
    label(fonts, text, frame().crumb, ui_color(Tone::Label.color(ui), 1.0))
}

/// The title's text.
pub fn title(fonts: &HudFonts, text: &str, ui: &UiPalette) -> impl Bundle + use<> {
    prose(fonts, text, frame().title, ui_color(Tone::Number.color(ui), 1.0))
}

/// The usual top left into a [`Frame::top_left`] row: the back ring with `button`, the breadcrumb over the title.
pub fn spawn_head(row: &mut ChildSpawnerCommands, fonts: &HudFonts, ui: &UiPalette, button: impl Bundle, crumb_text: &str, title_text: &str) {
    spawn_back(row, ui, button);
    row.spawn(frame().heading()).with_children(|col| {
        col.spawn(crumb(fonts, crumb_text, ui));
        col.spawn(title(fonts, title_text, ui));
    });
}

/// A pill of the top bar, at its size.
pub fn top_pill(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, text: &str, pill: Pill, extra: impl Bundle) -> Entity {
    let f = frame();
    spawn_pill(parent, fonts, text, f.pill_text, f.pill, None, pill, extra)
}

/// CLOSE at the top bar's end (Esc), with `button` on it.
pub fn spawn_close(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, button: impl Bundle) -> Entity {
    parent.spawn(Node { width: px(frame().pill_gap / 2.0), ..default() });
    top_pill(parent, fonts, "CLOSE", Pill::new(Tone::Teal), (button, Tip::key("CLOSE", "ESC")))
}

/// A plate's node: a thin border, a column of centred lines, the padding and gap.
pub fn panel_node() -> Node {
    let p = &frame().panel;
    Node {
        border: UiRect::all(px(1)),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::axes(px(p.pad_x), px(p.pad_y)),
        row_gap: px(p.gap),
        ..default()
    }
}

/// A plate's fill (the clear colour) and border (its accent).
pub fn panel_colors(clear: Color, accent: Color) -> (BackgroundColor, BorderColor) {
    let p = &frame().panel;
    (BackgroundColor(ui_color(clear, p.fill)), BorderColor::all(ui_color(accent, p.border)))
}

/// A plate's title.
pub fn panel_title(fonts: &HudFonts, text: &str, color: Color) -> impl Bundle + use<> {
    tracked(fonts, text, frame().panel.title, color, 0.3)
}
