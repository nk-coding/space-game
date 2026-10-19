//! Overlay scrollbars and glow room (Niklas, 14 Oct 2026: "the system is missing scroll bars in general (these
//! should be visible when scrolling, like these modern ones usually are)", and "the cut of glow looks weird": the
//! glow of a lit row or chip was cut in a hard box where its list ended).
//!
//! * **[`Scrollbar`]** on a node that scrolls or clips its rows (one line in its bundle) gets a thin rounded thumb
//!   over its right edge ([`Scrollbar::vertical`]) or along its bottom ([`Scrollbar::horizontal`];
//!   [`Scrollbar::outside`] puts it just under a strip, in the gap below it, so it covers nothing). The kit spawns
//!   the bar as the area's next sibling (its parent must have no border on the bar's side: the bar is placed from
//!   the corners) and places it every frame from the area's layout.
//! * **Glow room**: the kit lets the area show its content [`Scrollbar::room`] px past its edges (its
//!   `overflow_clip_margin`), so a glow at an edge fades out on its own instead of ending in a straight line. An
//!   edge the content is scrolled past cuts it right there instead ([`cut_scrolled_edges`]; Niklas, 14 Oct 2026:
//!   "the parts list overlaps this categories thingy": rows scrolled out of view showed in the room over the
//!   category tabs). A text cut by such an edge (across a vertical area's scrolled edge) is cut away whole, so no
//!   line shows half its letters (Niklas, 18 Oct 2026: the top row of the parts list showed its name cut through);
//!   the row's icon and outline are cut where the edge is. No fades over the edges, as in Chrome (Niklas, 14 Oct 2026: the panels' "weird gradient at top
//!   and bottom", a band of backdrop colour over whatever lay behind the panel): the bar is all that says an area
//!   scrolls ("there is no need for the UI to say that you can scroll via wheel"; the `MORE BELOW · WHEEL` cue is
//!   gone).
//! * **What scrolls**: the area's own [`ScrollPosition`] (its content size from the layout), or, for a list that
//!   shows a window of its rows in a fixed pool of row nodes, [`ScrollRows`]: the screen lays its rows out from the
//!   list's scroll in UI px ([`ScrollRows::lay`]: which rows the pool shows; it keeps its rows while the view stays
//!   inside them, see [`row_window`]) and the kit shifts the pool by the rest (the node's own [`ScrollPosition`]),
//!   so such a list scrolls by pixels too.
//! * **Smooth scrolling** (Niklas, 14 Oct 2026: "I want smooth scrolling like in chrome"): the kit owns the wheel
//!   for every [`Scrollbar`] area (the topmost one under the pointer that has more to show, unless another layer
//!   covers it; over a bar, its area). A wheel's notch glides ([`Glide`]): [`WHEEL_STEP`] UI px (at most
//!   [`PAGE_SHARE`] of the view) over [`GLIDE_SECONDS`] on an ease-out curve, and notches in quick succession move
//!   the running glide's target on instead of starting again. A touchpad's small pixel steps move the area at
//!   once, 1:1 under the fingers ([`wheel_move`]). A screen can glide an area somewhere with
//!   [`SmoothScroll::to`]; a place written directly (a drag on the bar, a list starting at the top) stops a glide.
//!   Always within the content: a glide ends at an end, never past it.
//! * **When it shows**: while the area scrolls (any change of its place: the wheel, a drag, a jump; touch has no
//!   hover, so this is what shows it there) and while the pointer is over the area, then [`HOLD_SECONDS`] more,
//!   then it fades out over [`FADE_OUT_SECONDS`] ([`bar_alpha`]). Never while the content fits.
//! * **Mouse and touch**: the bar is a [`Hot`] node (it takes the pointer from the rows under it); a press on the
//!   thumb drags it, a press on the track beside it jumps the thumb's middle there and drags on from there (at
//!   once: no glide). A finger dragged along the area itself (on its rows too) scrolls it under the finger
//!   ([`finger_scroll`], after [`FINGER_SLOP`]; no fling yet).
//! * **Look** (style guide "UI"): the label grey at low alpha, 4 px wide, 6 px while the pointer is on it or it is
//!   dragged, fully rounded, [`EDGE_INSET`] px in from the area's edge.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedNode, ComputedStackIndex, OverflowClipMargin, UiGlobalTransform, UiSystems};
use bevy::window::PrimaryWindow;

use super::{Hot, UiPointer};
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

/// An overlay scrollbar for the scrolling node it is on, and its smooth scrolling (see the module doc).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(SmoothScroll)]
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

    /// Another glow room (none where nothing in the area glows and something close by must stay clear).
    pub fn room(self, room: u8) -> Self {
        Self { room, ..self }
    }
}

/// A list that shows a window of its rows in a fixed pool of row nodes (the drydock's palette, the BLUEPRINTS and
/// BEHAVIORS lists, FLEET's cards): how long its content is and how far it is scrolled (UI px). The kit's wheel,
/// glide and bar drag write `scroll`; the screen may write it too (to start at the top); [`ScrollRows::lay`] keeps
/// both in range and says which rows the pool shows; the kit shifts the pool by `shift` (the node's own
/// [`ScrollPosition`], so the node needs `Overflow::scroll_*` on its axis, and one row node more than fit).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollRows {
    pub content: f32,
    pub scroll: f32,
    pub shift: f32,
    /// The pool's first row (kept while the view stays inside the pool).
    pub first: usize,
}

/// Which rows of a pooled list show: the pool holds `count` rows from `first` (every row in view, and as many past
/// it as the pool has room for) and is shifted back by `shift` px (the part above or before the view); `seen` are
/// the rows at least partly in view (from, to); the content's length and the scroll kept within it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowWindow {
    pub first: usize,
    pub count: usize,
    pub shift: f32,
    pub seen: (usize, usize),
    pub content: f32,
    pub scroll: f32,
}

/// The [`RowWindow`] of rows `lengths` long (UI px, along the list) with `gap` between them, scrolled by `scroll` in
/// a view `view` long, for a pool of `pool` row nodes that held rows from `held` last frame. The pool keeps its
/// rows while the view stays inside them: a row node that shows another row redraws it (a part icon is a mesh
/// built again, which showed as a blink while a list glided), so with a pool as long as the list nothing changes
/// rows while it scrolls, and a shorter pool moves on by as many rows as it can at once.
pub fn row_window(lengths: &[f32], gap: f32, scroll: f32, view: f32, pool: usize, held: usize) -> RowWindow {
    let total = lengths.len();
    let content = lengths.iter().sum::<f32>() + gap * total.saturating_sub(1) as f32;
    let scroll = scroll.clamp(0.0, (content - view).max(0.0));
    let tops: Vec<f32> = lengths.iter().scan(0.0, |top, h| Some(std::mem::replace(top, *top + h + gap))).collect();
    // In view: from the first row whose gap after it reaches the view, to the first row that starts below it.
    let from = tops.iter().zip(lengths).take_while(|(t, h)| *t + *h + gap <= scroll).count();
    let to = tops.iter().take_while(|t| **t < scroll + view).count().max(from);
    let pool = pool.max(to - from);
    let first = if held <= from && held + pool >= to {
        held
    } else if held > from {
        // Up: the pool reaches as far above as it can.
        to.saturating_sub(pool)
    } else {
        // Down: as far below.
        from.min(total.saturating_sub(pool))
    };
    let shift = scroll - tops.get(first).copied().unwrap_or(0.0);
    RowWindow { first, count: pool.min(total - first.min(total)), shift, seen: (from, to), content, scroll }
}

impl ScrollRows {
    /// Lays the list out from its rows' lengths (UI px) with `gap` between them in a view `view` long, for a pool of
    /// `pool` row nodes: keeps the content's length, the scroll within it, the pool's first row and its shift
    /// (written only when they change), and returns which rows show.
    pub fn lay(rows: &mut Mut<ScrollRows>, lengths: &[f32], gap: f32, view: f32, pool: usize) -> RowWindow {
        let w = row_window(lengths, gap, rows.scroll, view, pool, rows.first);
        let want = ScrollRows { content: w.content, scroll: w.scroll, shift: w.shift, first: w.first };
        if **rows != want {
            **rows = want;
        }
        w
    }
}

/// How far a wheel's notch scrolls (UI px; Chrome's step: 100 px at a scale of 1).
pub const WHEEL_STEP: f32 = 100.0;
/// A notch scrolls at most this share of the view (a small area doesn't jump past what it showed).
pub const PAGE_SHARE: f32 = 0.875;
/// A notch's glide (seconds; Chrome's smooth scrolling takes about as long).
pub const GLIDE_SECONDS: f32 = 0.18;
/// A pixel step of at least this many CSS px is a wheel's notch (browsers report a mouse wheel's notch in pixels,
/// 100 or 120 a notch); smaller ones are a touchpad's own smooth stream and move the area at once.
pub const PIXEL_NOTCH: f32 = 40.0;
/// Lines a notch reports: one natively, three in a browser that reports lines.
const LINES_PER_NOTCH: f32 = if cfg!(target_arch = "wasm32") { 3.0 } else { 1.0 };

/// What one wheel event does to an area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelMove {
    /// A notch: glide this far (UI px).
    Glide(f32),
    /// A touchpad's step: move this far at once (UI px).
    Direct(f32),
}

/// The move a wheel event of `delta` along the area's axis asks for (Bevy's sign: positive is up or left; in lines,
/// or physical px), with `css` physical px per CSS px (the window's scale factor), `ui` physical px per UI px and a
/// view `view` UI px long.
pub fn wheel_move(unit: MouseScrollUnit, delta: f32, css: f32, ui: f32, view: f32) -> WheelMove {
    let step = WHEEL_STEP.min(view * PAGE_SHARE).max(1.0);
    match unit {
        MouseScrollUnit::Line => WheelMove::Glide(-delta / LINES_PER_NOTCH * step),
        MouseScrollUnit::Pixel if (delta / css).abs() >= PIXEL_NOTCH => WheelMove::Glide(-delta / css / WHEEL_STEP * step),
        MouseScrollUnit::Pixel => WheelMove::Direct(-delta / ui),
    }
}

/// Chrome-like ease-out (cubic): fast at first, settling softly.
fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// A smooth scroll from one place to another over [`GLIDE_SECONDS`] (UI px along the area's axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glide {
    from: f32,
    to: f32,
    /// Seconds since it started.
    t: f32,
}

impl Glide {
    /// A glide from `at` to `target`, kept within `0..=max`; none when it is there already.
    pub fn new(at: f32, target: f32, max: f32) -> Option<Glide> {
        let to = target.clamp(0.0, max.max(0.0));
        ((to - at).abs() > 0.01).then_some(Glide { from: at, to, t: 0.0 })
    }

    /// A notch: `by` further on from the `running` glide's target (notches in quick succession add up), or from
    /// `at`; it starts from `at` again, so it runs on without a stop.
    pub fn by(running: Option<Glide>, at: f32, by: f32, max: f32) -> Option<Glide> {
        Glide::new(at, running.map_or(at, |g| g.to) + by, max)
    }

    /// Moves the whole glide by `by` (a touchpad's step during it), within `0..=max`.
    pub fn shift(&mut self, by: f32, max: f32) {
        self.from += by;
        self.to = (self.to + by).clamp(0.0, max.max(0.0));
    }

    /// `dt` seconds on (the content allowing `max` now): the place, and whether the glide is over.
    pub fn step(&mut self, dt: f32, max: f32) -> (f32, bool) {
        self.t += dt;
        self.to = self.to.clamp(0.0, max.max(0.0));
        let k = self.t / GLIDE_SECONDS;
        let at = self.from + (self.to - self.from) * ease_out(k);
        (at.clamp(0.0, max.max(0.0)), k >= 1.0)
    }
}

/// The smooth scrolling of a [`Scrollbar`] area (every one has it): the glide under way.
#[derive(Component, Debug, Default)]
pub struct SmoothScroll {
    glide: Option<Glide>,
    /// The place the glide wrote last: a different one means something else moved the area (the glide stops).
    wrote: Option<f32>,
    /// A place the screen asked to glide to.
    want: Option<f32>,
}

impl SmoothScroll {
    /// Glides the area to `target` (UI px; kept within its content), from the next frame.
    pub fn to(&mut self, target: f32) {
        self.want = Some(target);
    }

    /// A glide is under way (or asked for).
    pub fn gliding(&self) -> bool {
        self.glide.is_some() || self.want.is_some()
    }
}

/// The default glow room: the kit's glow is a box shadow with a 10 px blur, a Gaussian with sigma 10 px, so it
/// fades out over three sigma; at 16 px it still showed as a line under a lit chip (Niklas, 14 Oct 2026: "this
/// glow cutoff still looks weird").
pub const GLOW_ROOM: u8 = 30;
/// How long the bar stays after the last scroll or hover (seconds).
pub const HOLD_SECONDS: f32 = 1.0;
/// Then it fades out over this.
pub const FADE_OUT_SECONDS: f32 = 0.35;
/// It fades in this fast.
const FADE_IN_SECONDS: f32 = 0.1;
/// The thumb's width at rest and while the pointer is on the bar or drags it (UI px).
const THIN: f32 = 4.0;
const WIDE: f32 = 6.0;
/// The bar's hit area across (the thumb sits at its side next to the area's edge), and the gutter an area keeps for
/// it.
pub const BAR_HIT: f32 = 10.0;
/// The thumb's distance from the area's edge, and the track's from the area's ends.
pub const EDGE_INSET: f32 = 2.0;
/// The shortest thumb (a long list still gets one to grab).
const MIN_THUMB: f32 = 24.0;
/// The thumb's alpha (label grey) at rest and while the pointer is on the bar or drags it.
const ALPHA: f32 = 0.38;
const ALPHA_HOT: f32 = 0.62;

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

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (attach, (wheel, finger_scroll, drag, glide).chain().after(super::hit_test)).in_set(super::UiInputSet))
        .add_systems(Update, (shift_rows, show).in_set(super::UiStyleSet))
        .add_systems(PostUpdate, cut_scrolled_edges.in_set(ScrollClipSet).after(UiSystems::PostLayout));
}

/// After this the clips of scrolled content are final (UI icons read them).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScrollClipSet;

/// Cuts an area's content at an edge it is scrolled past (its glow room stays at the other edges): every node
/// inside gets one more clip rect, after bevy_ui has worked out the clips of the frame; in a vertical area a text
/// across that edge is cut away whole ([`cut_text`]).
fn cut_scrolled_edges(
    tracks: Query<&Track>,
    areas: Query<(&ComputedNode, &UiGlobalTransform, &ScrollPosition, Option<&ScrollRows>, &Children)>,
    children: Query<&Children>,
    texts: Query<(&ComputedNode, &UiGlobalTransform), With<Text>>,
    mut clips: Query<&mut CalculatedClip>,
) {
    for track in &tracks {
        let Ok((node, at, pos, rows, kids)) = areas.get(track.area) else { continue };
        let axis = track.bar.axis;
        let (view, content, scroll) = extent(axis, node, pos, rows);
        let (before, after) = (content - view > 0.5 && scroll > 0.5, content - view > 0.5 && scroll < content - view - 0.5);
        let Some(to_area) = at.try_inverse().filter(|_| before || after) else { continue };
        // In the area's own space (physical px about its centre): open across, cut along where it is scrolled.
        let half = node.size / 2.0;
        let (lo, hi) = (if before { -axis.of(half) } else { f32::NEG_INFINITY }, if after { axis.of(half) } else { f32::INFINITY });
        let cut = match axis {
            BarAxis::Vertical => Rect { min: Vec2::new(f32::NEG_INFINITY, lo), max: Vec2::new(f32::INFINITY, hi) },
            BarAxis::Horizontal => Rect { min: Vec2::new(lo, f32::NEG_INFINITY), max: Vec2::new(hi, f32::INFINITY) },
        };
        let mut stack: Vec<Entity> = kids.iter().collect();
        while let Some(e) = stack.pop() {
            if let Ok(mut clip) = clips.get_mut(e) {
                let rect = match texts.get(e) {
                    Ok((n, t)) if axis == BarAxis::Vertical => {
                        let mid = to_area.transform_point2(t.translation).y;
                        let (lo, hi) = cut_text(lo, hi, mid - n.size.y / 2.0, mid + n.size.y / 2.0);
                        Rect { min: Vec2::new(f32::NEG_INFINITY, lo), max: Vec2::new(f32::INFINITY, hi) }
                    }
                    _ => cut,
                };
                clip.push_rect(rect, to_area);
            }
            stack.extend(children.get(e).into_iter().flatten());
        }
    }
}

/// The cut along a vertical area (`lo` to `hi`, its own space) for a text from `top` to `bottom`: a text across an
/// edge is cut away whole (the cut moves past it), one inside it keeps the cut.
fn cut_text(lo: f32, hi: f32, top: f32, bottom: f32) -> (f32, f32) {
    let lo = if top < lo && bottom > lo { bottom } else { lo };
    let hi = if top < hi && bottom > hi { top } else { hi };
    (lo, hi.max(lo))
}

impl Scrollbar {
    /// What the kit adds to its area's node: the glow room and, for a bar inside the area, a gutter, kept whether or
    /// not the content scrolls (the layout never shifts): the content ends where the bar's hit area starts (Niklas,
    /// 14 Oct 2026: "the scrollbar overlaps with the content"). A node with a form for each layout class
    /// ([`super::ByClass`]) gets it again with each form.
    pub(super) fn fit(&self, node: &mut Node) {
        node.overflow_clip_margin = OverflowClipMargin::padding_box().with_margin(f32::from(self.room));
        if !self.outside {
            let side = match self.axis {
                BarAxis::Vertical => &mut node.padding.right,
                BarAxis::Horizontal => &mut node.padding.bottom,
            };
            *side = match *side {
                Val::Px(p) => Val::Px(p + BAR_HIT),
                _ => Val::Px(BAR_HIT),
            };
        }
    }
}

/// Gives every new [`Scrollbar`] area its glow room and gutter ([`Scrollbar::fit`]; a [`super::ByClass`] node gets
/// them with its form) and spawns its bar as its next sibling (it draws over it).
#[allow(clippy::type_complexity)]
fn attach(mut commands: Commands, mut areas: Query<(Entity, &Scrollbar, &ChildOf, &mut Node, Has<super::ByClass>), Added<Scrollbar>>) {
    for (area, bar, parent, mut node, by_class) in &mut areas {
        if !by_class {
            bar.fit(&mut node);
        }
        commands.spawn((
            Track { area, bar: *bar, idle: HOLD_SECONDS + FADE_OUT_SECONDS, alpha: 0.0, seen: None, grab: None },
            Hot::default(),
            Node { display: Display::None, position_type: PositionType::Absolute, ..default() },
            ChildOf(parent.parent()),
            children![(Thumb, Node { position_type: PositionType::Absolute, border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(Color::NONE))],
        ));
    }
}

/// An area's view, content and scroll along the bar (UI px): its [`ScrollRows`], else its own layout and
/// [`ScrollPosition`].
fn extent(axis: BarAxis, node: &ComputedNode, pos: &ScrollPosition, rows: Option<&ScrollRows>) -> (f32, f32, f32) {
    let k = node.inverse_scale_factor;
    let view = axis.of(node.size) * k;
    match rows {
        Some(r) => (view, r.content, r.scroll),
        None => (view, axis.of(node.content_size) * k, axis.of(pos.0)),
    }
}

/// Puts an area's scroll at `at` (its [`ScrollRows`], else its [`ScrollPosition`]), written only when it changes.
fn set_scroll(axis: BarAxis, pos: &mut Mut<ScrollPosition>, rows: Option<&mut Mut<ScrollRows>>, at: f32) {
    if let Some(rows) = rows {
        if rows.scroll != at {
            rows.scroll = at;
        }
        return;
    }
    set_axis(axis, pos, at);
}

/// Writes one axis of a [`ScrollPosition`] when it changes.
fn set_axis(axis: BarAxis, pos: &mut Mut<ScrollPosition>, at: f32) {
    let mut v = pos.0;
    match axis {
        BarAxis::Vertical => v.y = at,
        BarAxis::Horizontal => v.x = at,
    }
    if pos.0 != v {
        pos.0 = v;
    }
}

/// Is `e` the node `area` or inside it?
fn within(mut e: Entity, area: Entity, parents: &Query<&ChildOf>) -> bool {
    loop {
        if e == area {
            return true;
        }
        match parents.get(e) {
            Ok(p) => e = p.parent(),
            Err(_) => return false,
        }
    }
}

/// Every [`Scrollbar`] area, for the wheel and a finger's drag.
type Areas<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Scrollbar,
        &'static mut SmoothScroll,
        &'static ComputedNode,
        &'static UiGlobalTransform,
        &'static InheritedVisibility,
        &'static ComputedStackIndex,
        Option<&'static CalculatedClip>,
        &'static mut ScrollPosition,
        Option<&'static mut ScrollRows>,
    ),
    Without<Track>,
>;

/// The topmost area under `at` (physical px) with more to show than fits, unless a layer above it (a [`Hot`] node
/// `over` outside it) has the pointer.
fn area_under(at: Vec2, over: Option<Entity>, stacks: &Query<&ComputedStackIndex>, parents: &Query<&ChildOf>, areas: &Areas) -> Option<Entity> {
    let mut best: Option<(u32, Entity)> = None;
    for (e, bar, _, node, tf, vis, z, clip, pos, rows) in areas {
        let under = vis.get() && node.size.cmpgt(Vec2::ZERO).all() && node.contains_point(*tf, at) && clip.is_none_or(|c| c.contains_point(at));
        let (view, content, _) = extent(bar.axis, node, pos, rows);
        if under && content - view > 0.5 && best.is_none_or(|b| z.0 >= b.0) {
            best = Some((z.0, e));
        }
    }
    let (z, area) = best?;
    let covered = over.is_some_and(|h| stacks.get(h).is_ok_and(|s| s.0 > z) && !within(h, area, parents));
    (!covered).then_some(area)
}

/// A finger moves this far (logical px) along an area before its drag scrolls it (a shorter one stays a tap).
pub const FINGER_SLOP: f32 = 10.0;

/// A finger's drag that may scroll an area: the finger, the area, where the finger was and the area's scroll then
/// (UI px), and whether it scrolls yet.
#[derive(Clone, Copy, Debug)]
struct FingerScroll {
    finger: u64,
    area: Entity,
    from: Vec2,
    start: f32,
    engaged: bool,
}

/// A finger dragged along an area (on its rows too) scrolls it 1:1 under the finger, at once (docs/plans/android.md
/// 4.1: touch has no wheel). It starts once the finger has moved [`FINGER_SLOP`] along the area's axis (more than
/// across it, so a sideways slider in a list still drags); then the press on the row under it ends without a click.
#[allow(clippy::too_many_arguments)]
fn finger_scroll(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    mut pointer: ResMut<UiPointer>,
    tracks: Query<&Track>,
    stacks: Query<&ComputedStackIndex>,
    parents: Query<&ChildOf>,
    mut areas: Areas,
    mut drag: Local<Option<FingerScroll>>,
) {
    let Some(id) = pointer.finger() else {
        *drag = None;
        return;
    };
    let Some(at) = touches.get_pressed(id).map(|t| t.position() * window.scale_factor()) else {
        *drag = None;
        return;
    };
    if touches.just_pressed(id) || drag.is_some_and(|d| d.finger != id) {
        let on_bar = pointer.over.is_some_and(|e| tracks.get(e).is_ok());
        *drag = (!on_bar)
            .then(|| area_under(at, pointer.over, &stacks, &parents, &areas))
            .flatten()
            .and_then(|area| areas.get(area).ok().map(|(_, bar, _, node, _, _, _, _, pos, rows)| (area, extent(bar.axis, node, pos, rows).2)))
            .map(|(area, start)| FingerScroll { finger: id, area, from: at, start, engaged: false });
    }
    let Some(d) = drag.as_mut() else { return };
    let Ok((_, bar, mut smooth, node, _, _, _, _, mut pos, mut rows)) = areas.get_mut(d.area) else {
        *drag = None;
        return;
    };
    let axis = bar.axis;
    let moved = at - d.from;
    let across = match axis {
        BarAxis::Vertical => moved.x,
        BarAxis::Horizontal => moved.y,
    };
    if !d.engaged {
        let along = axis.of(moved);
        if along.abs() < FINGER_SLOP * window.scale_factor() || along.abs() <= across.abs() {
            return;
        }
        // From here on (no jump by the slop): the press on the row under the finger is no click.
        d.engaged = true;
        d.from = at;
        d.start = extent(axis, node, &pos, rows.as_deref()).2;
        pointer.cancel_press();
        return;
    }
    let (view, content, now) = extent(axis, node, &pos, rows.as_deref());
    let to = (d.start - axis.of(moved) * node.inverse_scale_factor).clamp(0.0, (content - view).max(0.0));
    if to != now {
        set_scroll(axis, &mut pos, rows.as_mut(), to);
    }
    if smooth.glide.is_some() || smooth.wrote != Some(to) {
        smooth.glide = None;
        smooth.wrote = Some(to);
    }
}

/// The wheel scrolls one [`Scrollbar`] area: the one whose bar is under the pointer, else the topmost one under it
/// with more to show than fits, unless a layer above it (a screen opened over it) has the pointer. A notch glides,
/// a touchpad's step moves at once ([`wheel_move`]); an area that scrolls sideways takes the wheel's up and down too.
#[allow(clippy::type_complexity)]
fn wheel(
    window: Single<&Window, With<PrimaryWindow>>,
    mut wheels: MessageReader<MouseWheel>,
    pointer: Res<UiPointer>,
    tracks: Query<&Track>,
    stacks: Query<&ComputedStackIndex>,
    parents: Query<&ChildOf>,
    mut areas: Areas,
) {
    let events: Vec<MouseWheel> = wheels.read().copied().collect();
    let Some(cursor) = window.physical_cursor_position().filter(|_| !events.is_empty()) else { return };
    let over = pointer.over;
    let target = over.and_then(|e| tracks.get(e).ok()).map(|t| t.area).or_else(|| area_under(cursor, over, &stacks, &parents, &areas));
    let Some(Ok((_, bar, mut smooth, node, _, _, _, _, mut pos, mut rows))) = target.map(|a| areas.get_mut(a)) else { return };
    let axis = bar.axis;
    let (view, content, start) = extent(axis, node, &pos, rows.as_deref());
    let max = (content - view).max(0.0);
    if smooth.wrote != Some(start) {
        smooth.glide = None;
    }
    let mut at = start;
    for ev in events {
        let d = match axis {
            BarAxis::Vertical => ev.y,
            BarAxis::Horizontal if ev.x.abs() > ev.y.abs() => ev.x,
            BarAxis::Horizontal => ev.y,
        };
        if d == 0.0 {
            continue;
        }
        match wheel_move(ev.unit, d, window.scale_factor(), node.inverse_scale_factor.recip(), view) {
            WheelMove::Glide(by) => smooth.glide = Glide::by(smooth.glide, at, by, max),
            WheelMove::Direct(by) => {
                at = (at + by).clamp(0.0, max);
                if let Some(g) = &mut smooth.glide {
                    g.shift(by, max);
                }
            }
        }
    }
    if at != start {
        set_scroll(axis, &mut pos, rows.as_mut(), at);
    }
    smooth.wrote = Some(at);
}

/// Runs every glide a frame on: one the screen asked for starts, one whose area something else moved stops.
#[allow(clippy::type_complexity)]
fn glide(time: Res<Time<Real>>, mut areas: Query<(&Scrollbar, &mut SmoothScroll, &ComputedNode, &mut ScrollPosition, Option<&mut ScrollRows>)>) {
    let dt = time.delta_secs();
    for (bar, mut smooth, node, mut pos, mut rows) in &mut areas {
        if !smooth.gliding() {
            continue;
        }
        let (view, content, at) = extent(bar.axis, node, &pos, rows.as_deref());
        let max = (content - view).max(0.0);
        if smooth.wrote != Some(at) {
            smooth.glide = None;
        }
        // A glide already on its way there runs on (asked again each frame, it would never get there).
        if let Some(target) = smooth.want.take()
            && smooth.glide.is_none_or(|g| (g.to - target.clamp(0.0, max)).abs() > 0.01)
        {
            smooth.glide = Glide::new(at, target, max);
        }
        let Some(mut g) = smooth.glide else { continue };
        let (now, done) = g.step(dt, max);
        smooth.glide = (!done).then_some(g);
        set_scroll(bar.axis, &mut pos, rows.as_mut(), now);
        smooth.wrote = Some(now);
    }
}

/// Shifts a pooled list's rows by what [`ScrollRows::lay`] left over (the part of its first row out of view).
fn shift_rows(mut lists: Query<(&Scrollbar, &ScrollRows, &mut ScrollPosition), Changed<ScrollRows>>) {
    for (bar, rows, mut pos) in &mut lists {
        set_axis(bar.axis, &mut pos, rows.shift);
    }
}

/// The track's length along the area (logical px).
fn track_len(axis: BarAxis, node: &ComputedNode) -> f32 {
    axis.of(node.size) * node.inverse_scale_factor - 2.0 * EDGE_INSET
}

/// The pointer: the mouse, else the first touch (physical px).
fn pointer(window: &Window, touches: &Touches) -> Option<Vec2> {
    window.physical_cursor_position().or_else(|| touches.iter().next().map(|t| t.position() * window.scale_factor()))
}

/// A press on a bar drags its thumb (on the track beside it: jumps its middle there first), at once (a glide
/// stops).
#[allow(clippy::type_complexity)]
fn drag(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    mut tracks: Query<(&mut Track, &Hot, &ComputedNode, &UiGlobalTransform)>,
    mut areas: Query<(&ComputedNode, &mut SmoothScroll, &mut ScrollPosition, Option<&mut ScrollRows>), Without<Track>>,
) {
    let at = pointer(&window, &touches);
    for (mut track, hot, tnode, ttf) in &mut tracks {
        if !hot.pressed {
            if track.grab.is_some() {
                track.grab = None;
            }
            continue;
        }
        let (Some(at), Ok((node, mut smooth, mut pos, mut rows))) = (at, areas.get_mut(track.area)) else { continue };
        let axis = track.bar.axis;
        let (view, content, scroll) = extent(axis, node, &pos, rows.as_deref());
        let len = track_len(axis, node);
        let Some((start, thumb_len)) = thumb(view, content, scroll, len) else { continue };
        // The pointer along the track from its start (logical px).
        let p = axis.of((at - (ttf.translation - tnode.size / 2.0)) * tnode.inverse_scale_factor);
        let grab = *track.grab.get_or_insert(if (start..=start + thumb_len).contains(&p) { p - start } else { thumb_len / 2.0 });
        let want = scroll_for(p - grab, view, content, len);
        if smooth.gliding() {
            (smooth.glide, smooth.want) = (None, None);
        }
        set_scroll(axis, &mut pos, rows.as_mut(), want);
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

/// Places every bar over its area, sizes and moves the thumb, fades the bar in while the area scrolls or is hovered
/// (out [`HOLD_SECONDS`] later); hides the bar while the content fits.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    time: Res<Time<Real>>,
    style: Res<Style>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut tracks: Query<(&mut Track, &Hot, &mut Node, &ChildOf, &Children)>,
    areas: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility, &ScrollPosition, Option<&ScrollRows>), Without<Track>>,
    parents: Query<(&ComputedNode, &UiGlobalTransform), Without<Track>>,
    mut thumbs: Query<(&mut Node, &mut BackgroundColor), (With<Thumb>, Without<Track>)>,
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

        let Some((start, thumb_len)) = bar else {
            if node.display != Display::None {
                node.display = Display::None;
            }
            (track.alpha, track.seen, track.grab) = (0.0, None, None);
            continue;
        };
        let hit = match (axis, track.bar.outside) {
            (BarAxis::Vertical, false) => Rect::new(r.max.x - BAR_HIT, r.min.y + EDGE_INSET, r.max.x, r.max.y - EDGE_INSET),
            (BarAxis::Vertical, true) => Rect::new(r.max.x, r.min.y + EDGE_INSET, r.max.x + BAR_HIT, r.max.y - EDGE_INSET),
            (BarAxis::Horizontal, false) => Rect::new(r.min.x + EDGE_INSET, r.max.y - BAR_HIT, r.max.x - EDGE_INSET, r.max.y),
            (BarAxis::Horizontal, true) => Rect::new(r.min.x + EDGE_INSET, r.max.y, r.max.x - EDGE_INSET, r.max.y + BAR_HIT),
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
        let off = if track.bar.outside { EDGE_INSET } else { BAR_HIT - EDGE_INSET - across };
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

    /// Runs a glide in steps of `dt` until it ends: the places it went through.
    fn run(mut g: Glide, dt: f32, max: f32) -> Vec<f32> {
        let mut out = Vec::new();
        loop {
            let (at, done) = g.step(dt, max);
            out.push(at);
            if done {
                return out;
            }
        }
    }

    /// A notch glides to its target in [`GLIDE_SECONDS`], fast at first and settling softly (ease-out), never
    /// going back or past.
    #[test]
    fn a_notch_glides_with_an_ease_out() {
        let dt = 1.0 / 60.0;
        let g = Glide::by(None, 0.0, WHEEL_STEP, 1000.0).unwrap();
        let path = run(g, dt, 1000.0);
        // About 0.18 s at 60 fps, ending on the target.
        assert_eq!(path.len(), (GLIDE_SECONDS / dt).ceil() as usize);
        assert_eq!(*path.last().unwrap(), WHEEL_STEP);
        // Between places on the way (the screenshots at 60 and 120 ms see it moving), always forward.
        assert!(path.windows(2).all(|w| w[1] >= w[0]), "{path:?}");
        assert!(path.iter().all(|&y| (0.0..=WHEEL_STEP).contains(&y)));
        // Ease-out: more than half way after a third of the time, the first step the longest.
        let third = path[(path.len() / 3) - 1];
        assert!(third > 0.5 * WHEEL_STEP && third < WHEEL_STEP, "{third}");
        let steps: Vec<f32> = std::iter::once(path[0]).chain(path.windows(2).map(|w| w[1] - w[0])).collect();
        assert!(steps.windows(2).all(|s| s[1] <= s[0] + 1e-4), "{steps:?}");
        assert!((ease_out(0.0), ease_out(1.0), ease_out(2.0)) == (0.0, 1.0, 1.0));
    }

    /// Notches in quick succession add up: the next one moves the running glide's target on, from where it is, so
    /// it never stops or starts again from zero speed.
    #[test]
    fn quick_notches_add_up() {
        let dt = 1.0 / 60.0;
        let mut g = Glide::by(None, 0.0, 100.0, 1000.0).unwrap();
        let mut path = vec![0.0];
        for _ in 0..3 {
            path.push(g.step(dt, 1000.0).0);
        }
        let (at, speed) = (path[3], path[3] - path[2]);
        let mut next = Glide::by(Some(g), at, 100.0, 1000.0).unwrap();
        assert_eq!(next.to, 200.0, "on from the running target, not from where it is");
        let (after, _) = next.step(dt, 1000.0);
        assert!(after - at >= speed, "it speeds up ({} after {speed})", after - at);
        let path = run(next, dt, 1000.0);
        assert_eq!(*path.last().unwrap(), 200.0);
        assert!(path.windows(2).all(|w| w[1] >= w[0]));
        // A notch the other way turns it around from its target.
        assert_eq!(Glide::by(Some(Glide::new(0.0, 200.0, 1000.0).unwrap()), 50.0, -100.0, 1000.0).unwrap().to, 100.0);
    }

    /// Glides stay within the content: they end at an end, not past it, and none starts at an end going out.
    #[test]
    fn glides_stop_at_the_ends() {
        let dt = 1.0 / 60.0;
        let g = Glide::by(None, 250.0, 100.0, 300.0).unwrap();
        let path = run(g, dt, 300.0);
        assert!(path.iter().all(|&y| y <= 300.0) && *path.last().unwrap() == 300.0, "{path:?}");
        assert_eq!(Glide::by(None, 300.0, 100.0, 300.0), None);
        assert_eq!(Glide::by(None, 0.0, -100.0, 300.0), None);
        assert_eq!(Glide::new(10.0, -50.0, 300.0).unwrap().to, 0.0);
        // Content that shrinks while it glides: kept inside the new end.
        let mut g = Glide::new(0.0, 300.0, 300.0).unwrap();
        g.step(dt, 300.0);
        let (at, _) = g.step(dt, 20.0);
        assert!(at <= 20.0);
        assert_eq!(run(g, dt, 20.0).last(), Some(&20.0));
        // A touchpad's step during a glide moves the whole glide, within the content.
        let mut g = Glide::new(0.0, 100.0, 300.0).unwrap();
        g.shift(250.0, 300.0);
        assert_eq!((g.from, g.to), (250.0, 300.0));
    }

    /// A wheel's notch glides by Chrome's step (less in a small area), a touchpad's small pixel steps move at once,
    /// 1:1 on the screen.
    #[test]
    fn notches_glide_and_touchpads_move_at_once() {
        // A notch down (Bevy: negative y) scrolls down by the step; a line per notch natively.
        assert_eq!(wheel_move(MouseScrollUnit::Line, -LINES_PER_NOTCH, 1.0, 1.0, 600.0), WheelMove::Glide(WHEEL_STEP));
        // Two notches in one frame: twice as far.
        assert_eq!(wheel_move(MouseScrollUnit::Line, 2.0 * LINES_PER_NOTCH, 1.0, 1.0, 600.0), WheelMove::Glide(-2.0 * WHEEL_STEP));
        // A small area: a notch moves less than it shows.
        assert_eq!(wheel_move(MouseScrollUnit::Line, -LINES_PER_NOTCH, 1.0, 1.0, 80.0), WheelMove::Glide(80.0 * PAGE_SHARE));
        // A browser's notch (100 CSS px, here at a scale factor of 2): one step, whatever the UI scale.
        assert_eq!(wheel_move(MouseScrollUnit::Pixel, -200.0, 2.0, 1.5, 600.0), WheelMove::Glide(WHEEL_STEP));
        assert_eq!(wheel_move(MouseScrollUnit::Pixel, 120.0, 1.0, 0.7, 600.0), WheelMove::Glide(-1.2 * WHEEL_STEP));
        // A touchpad's step: physical px to UI px, at once.
        assert_eq!(wheel_move(MouseScrollUnit::Pixel, -12.0, 1.0, 1.5, 600.0), WheelMove::Direct(8.0));
        assert_eq!(wheel_move(MouseScrollUnit::Pixel, 30.0, 2.0, 2.0, 600.0), WheelMove::Direct(-15.0));
    }

    /// A pooled list shows the rows in view from its scroll in pixels, one cut at each edge, and shifts the pool by
    /// what lies above the view.
    #[test]
    fn a_pooled_list_scrolls_by_pixels() {
        // Ten rows of 46 px with 4 px gaps (a pitch of 50) in a view of six rows (296 px), a pool of seven.
        let rows = [46.0; 10];
        let at = |scroll: f32, held: usize| row_window(&rows, 4.0, scroll, 296.0, 7, held);
        assert_eq!(at(0.0, 0), RowWindow { first: 0, count: 7, shift: 0.0, seen: (0, 6), content: 496.0, scroll: 0.0 });
        // 30 px down: the first row partly in view, a seventh cut at the bottom.
        assert_eq!(at(30.0, 0), RowWindow { first: 0, count: 7, shift: 30.0, seen: (0, 7), content: 496.0, scroll: 30.0 });
        // 48 px: the first row is gone, only its gap is above the view.
        assert_eq!((at(48.0, 0).first, at(48.0, 0).shift, at(48.0, 0).seen), (0, 48.0, (0, 7)));
        assert_eq!(at(50.0, 0).seen, (1, 7));
        // The pool keeps its rows while the view is inside them, then moves on as far as it can.
        assert_eq!((at(50.0, 0).first, at(50.0, 0).shift), (0, 50.0));
        let down = at(175.0, 0);
        assert_eq!((down.first, down.count, down.shift, down.seen), (3, 7, 25.0, (3, 10)));
        // Back up: the pool ends where the view does.
        let up = at(60.0, 3);
        assert_eq!((up.first, up.count, up.shift, up.seen), (1, 7, 10.0, (1, 8)));
        // Past the end: the last screenful.
        assert_eq!(at(900.0, 0), RowWindow { first: 3, count: 7, shift: 50.0, seen: (4, 10), content: 496.0, scroll: 200.0 });
        // A pool as long as the list never moves.
        assert_eq!(row_window(&rows, 4.0, 200.0, 296.0, 48, 0).first, 0);
        assert_eq!(row_window(&rows, 4.0, 200.0, 296.0, 48, 0).shift, 200.0);
        // Headings and rows of other heights (the drydock's palette).
        let mixed = [26.0, 42.0, 42.0, 26.0, 42.0];
        let w = row_window(&mixed, 3.0, 40.0, 60.0, 2, 0);
        assert_eq!((w.first, w.count, w.shift, w.seen, w.content), (1, 2, 11.0, (1, 3), 190.0));
        // A pool shorter than the view asks for: as many rows as are in view.
        assert_eq!(row_window(&rows, 4.0, 30.0, 296.0, 2, 0).count, 7);
        // Fewer rows than fit, or none: no scroll.
        assert_eq!(row_window(&rows[..3], 4.0, 80.0, 296.0, 7, 0), RowWindow { first: 0, count: 3, shift: 0.0, seen: (0, 3), content: 146.0, scroll: 0.0 });
        assert_eq!(row_window(&[], 4.0, 10.0, 296.0, 7, 4), RowWindow::default());
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

    /// A text across a scrolled edge is cut away whole; one inside the view, or with no edge cut, keeps the cut.
    #[test]
    fn a_text_across_an_edge_goes_whole() {
        assert_eq!(cut_text(-100.0, 100.0, -110.0, -90.0), (-90.0, 100.0));
        assert_eq!(cut_text(-100.0, 100.0, 90.0, 110.0), (-100.0, 90.0));
        assert_eq!(cut_text(-100.0, 100.0, -20.0, 20.0), (-100.0, 100.0));
        assert_eq!(cut_text(f32::NEG_INFINITY, 100.0, -120.0, -90.0), (f32::NEG_INFINITY, 100.0));
        assert_eq!(cut_text(-10.0, 10.0, -20.0, 20.0), (20.0, 20.0), "taller than the view: nothing shows");
    }
}
