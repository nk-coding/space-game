//! Overlay scrollbars and glow room (Niklas, 14 Oct 2026: "the system is missing scroll bars in general (these
//! should be visible when scrolling, like these modern ones usually are)", and "the cut of glow looks weird": the
//! glow of a lit row or chip was cut in a hard box where its list ended).
//!
//! * **[`Scrollbar`]** on a node that scrolls or clips its rows (one line in its bundle) gets a thin rounded thumb
//!   over its right edge ([`Scrollbar::vertical`]) or along its bottom ([`Scrollbar::horizontal`];
//!   [`Scrollbar::outside`] puts it just under a strip, in the gap below it, so it covers nothing). The kit spawns
//!   the bar as the area's next sibling (its parent must have no border on the bar's side: the bar is placed from
//!   the corners, like [`super::ScrollCue`]) and places it every frame from the area's layout.
//! * **Glow room**: the kit lets the area show its content [`Scrollbar::room`] px past its edges (its
//!   `overflow_clip_margin`), so a glow at an edge fades out on its own instead of ending in a straight line. Where
//!   content scrolls past an edge (a node's own [`ScrollPosition`]), a fade into the backdrop covers that room and
//!   [`FADE_DEPTH`] px inside the edge while there is more past it (the end's fade is left to a [`super::ScrollCue`]
//!   when the area has one).
//! * **What scrolls**: the area's own [`ScrollPosition`] (its content size from the layout), or, for a list that
//!   shows a window of its rows in a fixed pool of row nodes, [`ScrollRows`]: the screen writes which rows show,
//!   and takes the first row a drag asks for (no fades: such a list shows whole rows only).
//! * **When it shows**: while the area scrolls (any change of its place: the wheel, a drag, a jump; touch has no
//!   hover, so this is what shows it there) and while the pointer is over the area, then [`HOLD_SECONDS`] more,
//!   then it fades out over [`FADE_OUT_SECONDS`] ([`bar_alpha`]). Never while the content fits.
//! * **Mouse and touch**: the bar is a [`Hot`] node (it takes the pointer from the rows under it); a press on the
//!   thumb drags it, a press on the track beside it jumps the thumb's middle there and drags on from there.
//! * **Look** (style guide "UI"): the label grey at low alpha, 4 px wide, 6 px while the pointer is on it or it is
//!   dragged, fully rounded, [`EDGE_INSET`] px in from the area's edge.

use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, ComputedNode, LinearGradient, OverflowClipMargin, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use super::{Hot, ScrollCue};
use crate::style::{Style, ui_color};

/// Which way an area scrolls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAxis {
    Vertical,
    Horizontal,
}

impl BarAxis {
    fn of(self, v: Vec2) -> f32 {
        match self {
            BarAxis::Vertical => v.y,
            BarAxis::Horizontal => v.x,
        }
    }
}

/// An overlay scrollbar for the scrolling node it is on (see the module doc).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scrollbar {
    pub axis: BarAxis,
    /// Under the area (a sideways strip with room below it), not over its content.
    pub outside: bool,
    /// How far past its edges the area shows its content (UI px): room for a glow at an edge (default
    /// [`GLOW_ROOM`]).
    pub room: u8,
}

impl Scrollbar {
    /// Over the right edge of an area that scrolls up and down.
    pub fn vertical() -> Self {
        Self { axis: BarAxis::Vertical, outside: false, room: GLOW_ROOM }
    }

    /// Along the bottom of an area that scrolls sideways.
    pub fn horizontal() -> Self {
        Self { axis: BarAxis::Horizontal, outside: false, room: GLOW_ROOM }
    }

    /// Just under the area, in the gap below it.
    pub fn outside(self) -> Self {
        Self { outside: true, ..self }
    }

    /// Another glow room (less where something close by must not be covered while the area scrolls).
    pub fn room(self, room: u8) -> Self {
        Self { room, ..self }
    }
}

/// A list that pages a fixed pool of row nodes (the drydock's palette, the BLUEPRINTS and BEHAVIORS lists, FLEET's
/// cards): which rows show (the first, how many, of how many), written by the screen; `want` is the first row a
/// drag on the bar asks for ([`ScrollRows::take_want`], before the screen's own wheel).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollRows {
    pub first: usize,
    pub shown: usize,
    pub total: usize,
    pub want: Option<usize>,
}

impl ScrollRows {
    /// The first row a drag asks for, once.
    pub fn take_want(&mut self) -> Option<usize> {
        self.want.take()
    }

    /// Says which rows show (written only when it changes).
    pub fn show(rows: &mut Mut<ScrollRows>, first: usize, shown: usize, total: usize) {
        if (rows.first, rows.shown, rows.total) != (first, shown, total) {
            (rows.first, rows.shown, rows.total) = (first, shown, total);
        }
    }
}

/// The default glow room: the kit's glow (a 10 px blur) is gone 16 px past a lit row's edge.
pub const GLOW_ROOM: u8 = 16;
/// How far inside an edge its fade reaches (UI px).
pub const FADE_DEPTH: f32 = 26.0;
/// How long the bar stays after the last scroll or hover (seconds).
pub const HOLD_SECONDS: f32 = 1.0;
/// Then it fades out over this.
pub const FADE_OUT_SECONDS: f32 = 0.35;
/// It fades in this fast.
const FADE_IN_SECONDS: f32 = 0.1;
/// The thumb's width at rest and while the pointer is on the bar or drags it (UI px).
const THIN: f32 = 4.0;
const WIDE: f32 = 6.0;
/// The bar's hit area across (the thumb sits at its side next to the area's edge).
const HIT: f32 = 10.0;
/// The thumb's distance from the area's edge, and the track's from the area's ends.
pub const EDGE_INSET: f32 = 2.0;
/// The shortest thumb (a long list still gets one to grab).
const MIN_THUMB: f32 = 24.0;
/// The thumb's alpha (label grey) at rest and while the pointer is on the bar or drags it.
const ALPHA: f32 = 0.38;
const ALPHA_HOT: f32 = 0.62;
/// The fades' alpha (the backdrop) at their outer end.
const FADE_ALPHA: f32 = 0.95;

/// The thumb on a track of `track` px for a view of `view` over content of `content` scrolled by `scroll` (any one
/// unit: px or rows): its start along the track and its length. `None` when the content fits.
pub fn thumb(view: f32, content: f32, scroll: f32, track: f32) -> Option<(f32, f32)> {
    let range = content - view;
    if range <= 0.5 || view <= 0.0 || track <= 0.0 {
        return None;
    }
    let len = (track * view / content).clamp(MIN_THUMB.min(track), track);
    let t = (scroll / range).clamp(0.0, 1.0);
    Some((t * (track - len), len))
}

/// The scroll that puts the thumb's start at `start` (dragging; [`thumb`]'s inverse), clamped to the content.
pub fn scroll_for(start: f32, view: f32, content: f32, track: f32) -> f32 {
    let Some((_, len)) = thumb(view, content, 0.0, track) else { return 0.0 };
    let room = track - len;
    if room <= 0.0 {
        return 0.0;
    }
    (start / room).clamp(0.0, 1.0) * (content - view)
}

/// The bar's alpha after `dt` seconds from `alpha`, `idle` seconds after the area last scrolled or was hovered:
/// in quickly, held for [`HOLD_SECONDS`], then out over [`FADE_OUT_SECONDS`].
pub fn bar_alpha(alpha: f32, idle: f32, dt: f32) -> f32 {
    if idle < HOLD_SECONDS {
        (alpha + dt / FADE_IN_SECONDS).min(1.0)
    } else {
        (alpha - dt / FADE_OUT_SECONDS).max(0.0)
    }
}

/// The bar of a [`Scrollbar`] area (its hit area along the edge) and its state.
#[derive(Component, Debug)]
struct Track {
    area: Entity,
    bar: Scrollbar,
    /// The fades over the area's start and end edges (areas with their own [`ScrollPosition`]).
    fades: Option<[Entity; 2]>,
    /// Seconds since the area last scrolled or was hovered.
    idle: f32,
    alpha: f32,
    /// The scroll seen last frame (none yet: the first frame is no scroll).
    seen: Option<f32>,
    /// While dragged: where on the thumb it was grabbed (px from its start).
    grab: Option<f32>,
}

/// The thumb inside a [`Track`].
#[derive(Component)]
struct Thumb;

/// A fade over an edge of a [`Scrollbar`] area.
#[derive(Component)]
struct EdgeFade;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (attach, drag.after(super::hit_test)).in_set(super::UiInputSet))
        .add_systems(Update, show.in_set(super::UiStyleSet));
}

/// Gives every new [`Scrollbar`] area its glow room and spawns its fades and its bar as its next siblings (they draw
/// over it, the bar over the fades).
#[allow(clippy::type_complexity)]
fn attach(mut commands: Commands, style: Res<Style>, mut areas: Query<(Entity, &Scrollbar, &ChildOf, &mut Node, Has<ScrollRows>), Added<Scrollbar>>) {
    let back = style.background.outer();
    for (area, bar, parent, mut node, rows) in &mut areas {
        node.overflow_clip_margin = OverflowClipMargin::padding_box().with_margin(f32::from(bar.room));
        let fades = (!rows).then(|| {
            [false, true].map(|end| {
                // Opaque at the outer end, clear inside the area.
                let (from, to) = if end { (0.0, FADE_ALPHA) } else { (FADE_ALPHA, 0.0) };
                let stops = vec![ColorStop::percent(ui_color(back, from), 0), ColorStop::percent(ui_color(back, to), 100)];
                let gradient = match bar.axis {
                    BarAxis::Vertical => LinearGradient::to_bottom(stops),
                    BarAxis::Horizontal => LinearGradient::to_right(stops),
                };
                let node = Node { display: Display::None, position_type: PositionType::Absolute, ..default() };
                commands.spawn((EdgeFade, node, BackgroundGradient::from(gradient), ChildOf(parent.parent()))).id()
            })
        });
        commands.spawn((
            Track { area, bar: *bar, fades, idle: HOLD_SECONDS + FADE_OUT_SECONDS, alpha: 0.0, seen: None, grab: None },
            Hot::default(),
            Node { display: Display::None, position_type: PositionType::Absolute, ..default() },
            ChildOf(parent.parent()),
            children![(Thumb, Node { position_type: PositionType::Absolute, border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(Color::NONE))],
        ));
    }
}

/// An area's view, content and scroll along the bar (logical px, or rows for [`ScrollRows`]).
fn extent(axis: BarAxis, node: &ComputedNode, pos: Option<&ScrollPosition>, rows: Option<&ScrollRows>) -> (f32, f32, f32) {
    if let Some(r) = rows {
        return (r.shown as f32, r.total as f32, r.first as f32);
    }
    let k = node.inverse_scale_factor;
    (axis.of(node.size) * k, axis.of(node.content_size) * k, pos.map_or(0.0, |p| axis.of(p.0)))
}

/// The track's length along the area (logical px).
fn track_len(axis: BarAxis, node: &ComputedNode) -> f32 {
    axis.of(node.size) * node.inverse_scale_factor - 2.0 * EDGE_INSET
}

/// The pointer: the mouse, else the first touch (physical px).
fn pointer(window: &Window, touches: &Touches) -> Option<Vec2> {
    window.physical_cursor_position().or_else(|| touches.iter().next().map(|t| t.position() * window.scale_factor()))
}

/// A press on a bar drags its thumb (on the track beside it: jumps its middle there first): the area's
/// [`ScrollPosition`], or the first row its [`ScrollRows`] asks for.
#[allow(clippy::type_complexity)]
fn drag(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    mut tracks: Query<(&mut Track, &Hot, &ComputedNode, &UiGlobalTransform)>,
    mut areas: Query<(&ComputedNode, Option<&mut ScrollPosition>, Option<&mut ScrollRows>), Without<Track>>,
) {
    let at = pointer(&window, &touches);
    for (mut track, hot, tnode, ttf) in &mut tracks {
        if !hot.pressed {
            if track.grab.is_some() {
                track.grab = None;
            }
            continue;
        }
        let (Some(at), Ok((node, pos, rows))) = (at, areas.get_mut(track.area)) else { continue };
        let axis = track.bar.axis;
        let (view, content, scroll) = extent(axis, node, pos.as_deref(), rows.as_deref());
        let len = track_len(axis, node);
        let Some((start, thumb_len)) = thumb(view, content, scroll, len) else { continue };
        // The pointer along the track from its start (logical px).
        let p = axis.of((at - (ttf.translation - tnode.size / 2.0)) * tnode.inverse_scale_factor);
        let grab = *track.grab.get_or_insert(if (start..=start + thumb_len).contains(&p) { p - start } else { thumb_len / 2.0 });
        let want = scroll_for(p - grab, view, content, len);
        match (pos, rows) {
            (_, Some(mut rows)) => {
                let first = want.round() as usize;
                if first != rows.first && rows.want != Some(first) {
                    rows.want = Some(first);
                }
            }
            (Some(mut pos), None) => {
                let mut v = pos.0;
                match axis {
                    BarAxis::Vertical => v.y = want,
                    BarAxis::Horizontal => v.x = want,
                }
                if pos.0 != v {
                    pos.0 = v;
                }
            }
            (None, None) => {}
        }
    }
}

/// Writes a node's display and absolute rectangle (logical px) when they differ.
fn place(node: &mut Node, show: bool, rect: Rect) {
    let want = Node {
        display: if show { Display::Flex } else { Display::None },
        position_type: PositionType::Absolute,
        left: px(rect.min.x),
        top: px(rect.min.y),
        width: px(rect.width()),
        height: px(rect.height()),
        ..default()
    };
    if *node != want {
        *node = want;
    }
}

/// Places every bar and its fades over the area, sizes and moves the thumb, fades the bar in while the area scrolls
/// or is hovered (out [`HOLD_SECONDS`] later), shows a fade over each edge with more past it; hides the bar while
/// the content fits.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    time: Res<Time<Real>>,
    style: Res<Style>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut tracks: Query<(&mut Track, &Hot, &mut Node, &ChildOf, &Children)>,
    areas: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility, Option<&ScrollPosition>, Option<&ScrollRows>), Without<Track>>,
    parents: Query<(&ComputedNode, &UiGlobalTransform), Without<Track>>,
    mut thumbs: Query<(&mut Node, &mut BackgroundColor), (With<Thumb>, Without<Track>, Without<EdgeFade>)>,
    mut fades: Query<&mut Node, (With<EdgeFade>, Without<Track>, Without<Thumb>)>,
    cues: Query<&ScrollCue>,
) {
    let dt = time.delta_secs();
    let cursor = window.physical_cursor_position();
    for (mut track, hot, mut node, parent, children) in &mut tracks {
        let (Ok((area, at, vis, pos, rows)), Ok((p, p_at))) = (areas.get(track.area), parents.get(parent.parent())) else { continue };
        let axis = track.bar.axis;
        let (view, content, scroll) = extent(axis, area, pos, rows);
        let len = track_len(axis, area);
        let bar = thumb(view, content, scroll, len).filter(|_| vis.get());
        // The area in its parent (logical px; the parent's border is outside its children's box).
        let k = area.inverse_scale_factor;
        let min = ((at.translation - area.size / 2.0) - (p_at.translation - p.size / 2.0)) * k - p.border().min_inset * k;
        let r = Rect::from_corners(min, min + area.size * k);

        // The fades: over the start edge while scrolled from it, over the end edge while more lies past it.
        if let Some([start_fade, end_fade]) = track.fades {
            let room = f32::from(track.bar.room);
            let (more_before, more_after) = (bar.is_some() && scroll > 0.5, bar.is_some() && scroll < content - view - 0.5);
            let cue = cues.iter().any(|c| c.0 == track.area);
            let (start_rect, end_rect) = match axis {
                BarAxis::Vertical => (
                    Rect::new(r.min.x, r.min.y - room, r.max.x, r.min.y + FADE_DEPTH),
                    Rect::new(r.min.x, r.max.y - FADE_DEPTH, r.max.x, r.max.y + room),
                ),
                BarAxis::Horizontal => (
                    Rect::new(r.min.x - room, r.min.y, r.min.x + FADE_DEPTH, r.max.y),
                    Rect::new(r.max.x - FADE_DEPTH, r.min.y, r.max.x + room, r.max.y),
                ),
            };
            for (e, show, rect) in [(start_fade, more_before, start_rect), (end_fade, more_after && !cue, end_rect)] {
                if let Ok(mut n) = fades.get_mut(e) {
                    place(&mut n, show, rect);
                }
            }
        }

        let Some((start, thumb_len)) = bar else {
            if node.display != Display::None {
                node.display = Display::None;
            }
            (track.alpha, track.seen, track.grab) = (0.0, None, None);
            continue;
        };
        let hit = match (axis, track.bar.outside) {
            (BarAxis::Vertical, false) => Rect::new(r.max.x - HIT, r.min.y + EDGE_INSET, r.max.x, r.max.y - EDGE_INSET),
            (BarAxis::Vertical, true) => Rect::new(r.max.x, r.min.y + EDGE_INSET, r.max.x + HIT, r.max.y - EDGE_INSET),
            (BarAxis::Horizontal, false) => Rect::new(r.min.x + EDGE_INSET, r.max.y - HIT, r.max.x - EDGE_INSET, r.max.y),
            (BarAxis::Horizontal, true) => Rect::new(r.min.x + EDGE_INSET, r.max.y, r.max.x - EDGE_INSET, r.max.y + HIT),
        };
        place(&mut node, true, hit);

        // Shown while it scrolls, while the pointer is over the area or the bar, while dragged; then held and faded.
        let scrolled = track.seen.is_some_and(|s| (s - scroll).abs() > 1e-3);
        track.seen = Some(scroll);
        let hovered = cursor.is_some_and(|c| area.contains_point(*at, c));
        let hot_bar = hot.hovered || hot.pressed;
        track.idle = if scrolled || hovered || hot_bar { 0.0 } else { track.idle + dt };
        track.alpha = bar_alpha(track.alpha, track.idle, dt);

        let across = if hot_bar { WIDE } else { THIN };
        // The thumb [`EDGE_INSET`] from the area's edge: inside it at the hit area's far side, outside at its near one.
        let off = if track.bar.outside { EDGE_INSET } else { HIT - EDGE_INSET - across };
        let (l, t, w, h) = match axis {
            BarAxis::Vertical => (px(off), px(start), px(across), px(thumb_len)),
            BarAxis::Horizontal => (px(start), px(off), px(thumb_len), px(across)),
        };
        let color = ui_color(style.ui.label, track.alpha * if hot_bar { ALPHA_HOT } else { ALPHA });
        for child in children {
            let Ok((mut n, mut bg)) = thumbs.get_mut(*child) else { continue };
            if n.left != l || n.top != t || n.width != w || n.height != h {
                (n.left, n.top, n.width, n.height) = (l, t, w, h);
            }
            if bg.0 != color {
                bg.0 = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thumb is the visible share of the track and moves with the scroll from end to end.
    #[test]
    fn the_thumb_shows_the_view_and_the_place() {
        // A quarter of the content in view: a quarter of the track.
        assert_eq!(thumb(100.0, 400.0, 0.0, 200.0), Some((0.0, 50.0)));
        // Scrolled to the end: the thumb at the end; halfway: halfway along the room it has.
        assert_eq!(thumb(100.0, 400.0, 300.0, 200.0), Some((150.0, 50.0)));
        assert_eq!(thumb(100.0, 400.0, 150.0, 200.0), Some((75.0, 50.0)));
        // Past the ends (a scroll not yet clamped): kept on the track.
        assert_eq!(thumb(100.0, 400.0, 900.0, 200.0), Some((150.0, 50.0)));
        assert_eq!(thumb(100.0, 400.0, -20.0, 200.0), Some((0.0, 50.0)));
        // Content that fits (or half a pixel more): no bar.
        assert_eq!(thumb(100.0, 100.0, 0.0, 200.0), None);
        assert_eq!(thumb(100.0, 100.4, 0.0, 200.0), None);
        assert_eq!(thumb(0.0, 100.0, 0.0, 200.0), None);
        // A very long list: the thumb keeps a length to grab.
        assert_eq!(thumb(10.0, 10_000.0, 0.0, 200.0), Some((0.0, MIN_THUMB)));
        // Rows: 6 of 9 shown from the fourth.
        assert_eq!(thumb(6.0, 9.0, 3.0, 300.0), Some((100.0, 200.0)));
    }

    /// Dragging the thumb to a place scrolls to what [`thumb`] would draw there.
    #[test]
    fn a_drag_scrolls_to_where_the_thumb_is() {
        for scroll in [0.0, 37.5, 150.0, 300.0] {
            let (start, _) = thumb(100.0, 400.0, scroll, 200.0).unwrap();
            assert!((scroll_for(start, 100.0, 400.0, 200.0) - scroll).abs() < 1e-3, "{scroll}");
        }
        // Past the ends of the track: the ends of the content.
        assert_eq!(scroll_for(-40.0, 100.0, 400.0, 200.0), 0.0);
        assert_eq!(scroll_for(500.0, 100.0, 400.0, 200.0), 300.0);
        // Content that fits never scrolls.
        assert_eq!(scroll_for(50.0, 100.0, 80.0, 200.0), 0.0);
        // Rows: the thumb two thirds along its room shows from the third row.
        assert_eq!(scroll_for(67.0, 6.0, 9.0, 300.0).round(), 2.0);
    }

    /// In fast, held for a second after the last scroll or hover, then out; gone [`FADE_OUT_SECONDS`] later.
    #[test]
    fn the_bar_fades_out_a_second_after_scrolling() {
        let dt = 1.0 / 60.0;
        // Scrolling: fully in within the fade-in.
        let mut a = 0.0;
        for _ in 0..7 {
            a = bar_alpha(a, 0.0, dt);
        }
        assert_eq!(a, 1.0);
        // Then idle: held for the hold time.
        let mut idle = 0.0;
        while idle + dt < HOLD_SECONDS {
            idle += dt;
            a = bar_alpha(a, idle, dt);
            assert_eq!(a, 1.0, "{idle}");
        }
        // Then out: partly faded halfway through the fade, gone after it.
        let mut half = None;
        let mut t = 0.0;
        while t < FADE_OUT_SECONDS + 2.0 * dt {
            idle += dt;
            t += dt;
            a = bar_alpha(a, idle, dt);
            if half.is_none() && t >= FADE_OUT_SECONDS / 2.0 {
                half = Some(a);
            }
        }
        assert!(half.is_some_and(|h| h > 0.3 && h < 0.7), "{half:?}");
        assert_eq!(a, 0.0);
    }
}
