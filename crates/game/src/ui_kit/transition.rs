//! Screen transitions (Niklas, 13 Oct 2026: "the stuff on the left and right should slide in … nothing should be
//! hidden abruptly"): a screen opens with its parts sliding in from their edges of the window while they fade in,
//! slightly staggered, and closes the same way backwards, a little faster.
//!
//! * **[`ScreenFx`]** on a screen's UI root: the screen asks for open or closed ([`show_screen`]; [`snap_screen`]
//!   jumps without a transition), the kit runs the transition and owns the root's `Visibility` (hidden once it has
//!   closed). [`ScreenFx::alpha`] is the screen's fade for its world-space elements (FX lines, meshes);
//!   [`ScreenFx::visible`] says whether anything of it still shows (its world elements draw while it closes).
//! * **[`ScreenPart`]** on a panel or bar inside the root: where it comes from ([`Edge`]: out past that window
//!   edge, or `Fade` in place) and its place in the stagger (`order`, 0 first). Everything not inside a part fades
//!   with the root.
//! * **How**: transforms and alpha only. A part moves by its `UiTransform` (no layout), colours (texts, backgrounds,
//!   borders, glows) are written with the fade's alpha and given back exactly when it ends (a colour a screen system
//!   writes meanwhile becomes the one faded), UI icons (`ui_icons`, world meshes over their nodes) follow their node
//!   and draw with their part's own atlas material whose alpha is the part's (`atlas::Atlas::group_material`). Only
//!   while a transition runs is anything written.
//! * **[`WorldCover`]**: how much the open screens marked [`ScreenFx::covering`] hide the world behind them (the
//!   shipyard fades out under them).
//!
//! Timing (defaults for review): a part slides for [`SLIDE_SECONDS`] with an ease-out, each order starts
//! [`STAGGER_SECONDS`] later (at most [`MAX_ORDER`]); a close takes [`EXIT_SECONDS`] in all, the last part in going
//! first. Home's camera waits [`EXIT_SECONDS`] before it flies out of a closing screen.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiSystems, UiTransform, Val2};
use bevy::window::PrimaryWindow;

use crate::atlas::{Atlas, AtlasMaterial, set_alpha};
use crate::ui_icons::UiIcon;

/// How long one part slides (seconds).
pub const SLIDE_SECONDS: f32 = 0.38;
/// How much later each order starts.
pub const STAGGER_SECONDS: f32 = 0.05;
/// The last order.
pub const MAX_ORDER: u8 = 3;
/// A whole opening.
pub const ENTER_SECONDS: f32 = SLIDE_SECONDS + MAX_ORDER as f32 * STAGGER_SECONDS;
/// A whole closing.
pub const EXIT_SECONDS: f32 = 0.32;
/// How far past the window's edge a part starts (UI px).
const PAST_EDGE: f32 = 12.0;

/// Where a [`ScreenPart`] comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
    /// It fades in place.
    Fade,
}

/// A part of a screen that slides in from its [`Edge`] and fades in when the screen opens.
#[derive(Component, Clone, Debug)]
pub struct ScreenPart {
    pub edge: Edge,
    /// Its place in the stagger: 0 first, at most [`MAX_ORDER`].
    pub order: u8,
    /// The offset it has now (UI px).
    applied: Vec2,
    /// Its icons' material (made when it first has an icon).
    material: Option<Handle<AtlasMaterial>>,
}

impl ScreenPart {
    pub fn new(edge: Edge, order: u8) -> Self {
        Self { edge, order: order.min(MAX_ORDER), applied: Vec2::ZERO, material: None }
    }
}

/// A screen root with an enter / exit transition (see the module doc).
#[derive(Component, Clone, Debug)]
pub struct ScreenFx {
    open: bool,
    /// Along the whole transition: 0 closed, 1 open.
    t: f32,
    /// Nothing left to write until it is asked to change.
    settled: bool,
    /// The screen hides the world behind it ([`WorldCover`]).
    covers: bool,
    material: Option<Handle<AtlasMaterial>>,
}

impl ScreenFx {
    /// Closed (spawn screens like this, hidden).
    pub fn closed() -> Self {
        Self { open: false, t: 0.0, settled: false, covers: false, material: None }
    }

    /// The screen hides the world behind it while open.
    pub fn covering(self) -> Self {
        Self { covers: true, ..self }
    }

    /// Asked to be open (it may still be on its way).
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Something of it shows: open, opening or closing.
    pub fn visible(&self) -> bool {
        self.open || self.t > 0.0
    }

    /// The screen's fade (0..1): its world-space elements draw with it.
    pub fn alpha(&self) -> f32 {
        part_progress(self.t, 0)
    }

}

/// Opens or closes a screen with its transition (written only when it changes).
pub fn show_screen(fx: &mut Mut<ScreenFx>, open: bool) {
    if fx.open != open {
        fx.open = open;
        fx.settled = false;
    }
}

/// Opens or closes a screen at once, without a transition (a switch that changes the UI scale, or the world
/// under it, would make a transition jump).
pub fn snap_screen(fx: &mut Mut<ScreenFx>, open: bool) {
    let t = if open { 1.0 } else { 0.0 };
    if fx.open != open || fx.t != t {
        fx.open = open;
        fx.t = t;
        fx.settled = false;
    }
}

/// How much the open covering screens hide the world (0: not at all, 1: fully).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct WorldCover(pub f32);

/// The colours a fade took over: what they are without the fade, and what it wrote last.
#[derive(Clone, Debug, PartialEq)]
struct Colors {
    text: Option<Color>,
    bg: Option<Color>,
    border: Option<[Color; 4]>,
    shadow: Option<Vec<Color>>,
}

impl Colors {
    fn times(&self, a: f32) -> Self {
        let f = |c: Color| c.with_alpha(c.alpha() * a);
        Self {
            text: self.text.map(f),
            bg: self.bg.map(f),
            border: self.border.map(|b| b.map(f)),
            shadow: self.shadow.as_ref().map(|s| s.iter().map(|c| f(*c)).collect()),
        }
    }
}

struct Faded {
    base: Colors,
    written: Colors,
}

#[derive(Resource, Default)]
struct FadeBases(HashMap<Entity, Faded>);

/// The material each UI icon draws with while its screen fades (its part's); `ui_icons` reads it.
#[derive(Resource, Default)]
pub struct IconMaterials(pub HashMap<Entity, Handle<AtlasMaterial>>);

/// Where along its slide a part of `order` is at `t` (0..1 along the whole transition), eased: 0 out, 1 in place.
pub fn part_progress(t: f32, order: u8) -> f32 {
    let p = ((t * ENTER_SECONDS - order.min(MAX_ORDER) as f32 * STAGGER_SECONDS) / SLIDE_SECONDS).clamp(0.0, 1.0);
    1.0 - (1.0 - p).powi(3)
}

/// One frame of a transition: `t` moves to 1 in [`ENTER_SECONDS`] while open, to 0 in [`EXIT_SECONDS`] while
/// closed.
pub fn step(t: f32, open: bool, dt: f32) -> f32 {
    if open { (t + dt / ENTER_SECONDS).min(1.0) } else { (t - dt / EXIT_SECONDS).max(0.0) }
}

/// How far a part whose rectangle is `rect` (UI px, y down) moves to be past its `edge` of a `window` (UI px).
pub fn slide_offset(edge: Edge, rect: Rect, window: Vec2) -> Vec2 {
    match edge {
        Edge::Left => Vec2::new(-(rect.max.x + PAST_EDGE), 0.0),
        Edge::Right => Vec2::new(window.x - rect.min.x + PAST_EDGE, 0.0),
        Edge::Top => Vec2::new(0.0, -(rect.max.y + PAST_EDGE)),
        Edge::Bottom => Vec2::new(0.0, window.y - rect.min.y + PAST_EDGE),
        Edge::Fade => Vec2::ZERO,
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WorldCover>()
        .init_resource::<FadeBases>()
        .init_resource::<IconMaterials>()
        .add_systems(PostUpdate, animate.before(UiSystems::Layout));
}

type NodeParts<'a> = (
    Option<&'a mut ScreenPart>,
    Option<&'a ComputedNode>,
    Option<&'a UiGlobalTransform>,
    Option<&'a mut UiTransform>,
    Option<&'a mut TextColor>,
    Option<&'a mut BackgroundColor>,
    Option<&'a mut BorderColor>,
    Option<&'a mut BoxShadow>,
    Has<UiIcon>,
);

/// Runs every screen's transition (see the module doc).
#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    atlas: Res<Atlas>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
    mut roots: Query<(Entity, &mut ScreenFx, &mut Visibility)>,
    children: Query<&Children>,
    mut nodes: Query<NodeParts>,
    mut faded: ResMut<FadeBases>,
    mut icons: ResMut<IconMaterials>,
    mut cover: ResMut<WorldCover>,
) {
    let dt = time.delta_secs().min(0.1);
    let window_px = window.physical_size().as_vec2();
    let mut covered = 0.0f32;
    for (root, mut fx, mut vis) in &mut roots {
        let target = if fx.open { 1.0 } else { 0.0 };
        if !(fx.settled && fx.t == target) {
            let t = step(fx.t, fx.open, dt);
            fx.t = t;
            let root_mat = fx.material.get_or_insert_with(|| atlas.group_material(&mut materials)).clone();
            let done = t == target;
            let mut walked = Vec::new();
            let mut stack = vec![(root, part_progress(t, 0), root_mat.clone())];
            set_alpha(&mut materials, &root_mat, part_progress(t, 0));
            while let Some((e, mut alpha, mut mat)) = stack.pop() {
                if let Ok((part, node, gtf, utf, text, bg, border, shadow, icon)) = nodes.get_mut(e) {
                    if let Some(mut part) = part {
                        alpha = part_progress(t, part.order);
                        mat = part.material.get_or_insert_with(|| atlas.group_material(&mut materials)).clone();
                        set_alpha(&mut materials, &mat, alpha);
                        if let (Some(node), Some(gtf), Some(mut utf)) = (node, gtf, utf) {
                            let k = node.inverse_scale_factor;
                            // Its rectangle without the offset it has now (UI px).
                            let centre = gtf.translation * k - part.applied;
                            let rect = Rect::from_center_size(centre, node.size * k);
                            let offset = if node.size.min_element() > 0.0 { slide_offset(part.edge, rect, window_px * k) * (1.0 - alpha) } else { Vec2::ZERO };
                            let v = Val2::px(offset.x, offset.y);
                            if utf.translation != v {
                                utf.translation = v;
                            }
                            part.applied = offset;
                        }
                    }
                    if text.is_some() || bg.is_some() || border.is_some() || shadow.is_some() {
                        fade(&mut faded.0, e, alpha, text, bg, border, shadow);
                        if done && target == 1.0 {
                            walked.push(e);
                        }
                    }
                    if icon && icons.0.get(&e) != Some(&mat) {
                        icons.0.insert(e, mat.clone());
                    }
                }
                if let Ok(kids) = children.get(e) {
                    stack.extend(kids.iter().map(|k| (k, alpha, mat.clone())));
                }
            }
            // Open: the colours are their own again.
            for e in walked {
                faded.0.remove(&e);
            }
            let want = if fx.visible() { Visibility::Inherited } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
            fx.settled = done;
        } else {
            let want = if fx.open { Visibility::Inherited } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
        }
        if fx.covers {
            covered = covered.max(fx.alpha());
        }
    }
    if cover.0 != covered {
        cover.0 = covered;
    }
}

/// Writes a node's colours at `alpha` of what they are without the fade (a colour someone else wrote since the
/// last frame of the fade is taken as the new one).
fn fade(
    faded: &mut HashMap<Entity, Faded>,
    e: Entity,
    alpha: f32,
    text: Option<Mut<TextColor>>,
    bg: Option<Mut<BackgroundColor>>,
    border: Option<Mut<BorderColor>>,
    shadow: Option<Mut<BoxShadow>>,
) {
    let current = Colors {
        text: text.as_ref().map(|c| c.0),
        bg: bg.as_ref().map(|c| c.0),
        border: border.as_ref().map(|b| [b.top, b.right, b.bottom, b.left]),
        shadow: shadow.as_ref().map(|s| s.0.iter().map(|s| s.color).collect()),
    };
    let entry = faded.entry(e).or_insert_with(|| Faded { base: current.clone(), written: current.clone() });
    // Per colour: one written by someone else since is the new one.
    let (base, written) = (&mut entry.base, &entry.written);
    if written.text != current.text {
        base.text = current.text;
    }
    if written.bg != current.bg {
        base.bg = current.bg;
    }
    if written.border != current.border {
        base.border = current.border;
    }
    if written.shadow != current.shadow {
        base.shadow.clone_from(&current.shadow);
    }
    let want = entry.base.times(alpha);
    if want.text != current.text
        && let (Some(mut c), Some(w)) = (text, want.text)
    {
        c.0 = w;
    }
    if want.bg != current.bg
        && let (Some(mut c), Some(w)) = (bg, want.bg)
    {
        c.0 = w;
    }
    if want.border != current.border
        && let (Some(mut b), Some([top, right, bottom, left])) = (border, want.border)
    {
        *b = BorderColor { top, right, bottom, left };
    }
    if want.shadow != current.shadow
        && let (Some(mut s), Some(w)) = (shadow, want.shadow.as_ref())
    {
        for (style, c) in s.0.iter_mut().zip(w) {
            style.color = *c;
        }
    }
    entry.written = want;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An opening takes ENTER_SECONDS at any frame rate, a closing EXIT_SECONDS; parts arrive in their order, eased
    /// out (fast first, settling), and leave last-in first.
    #[test]
    fn a_transition_runs_its_time_and_staggers() {
        for fps in [30.0, 60.0, 144.0] {
            let (mut t, mut frames) = (0.0, 0);
            while t < 1.0 {
                t = step(t, true, 1.0 / fps);
                frames += 1;
            }
            assert!((frames as f32 / fps - ENTER_SECONDS).abs() <= 1.0 / fps + 1e-4, "{fps}");
            let mut frames = 0;
            while t > 0.0 {
                t = step(t, false, 1.0 / fps);
                frames += 1;
            }
            assert!((frames as f32 / fps - EXIT_SECONDS).abs() <= 1.0 / fps + 1e-4, "{fps}");
        }
        assert_eq!((part_progress(0.0, 0), part_progress(1.0, 0), part_progress(1.0, MAX_ORDER)), (0.0, 1.0, 1.0));
        // Midway the first part is further than the later ones; an ease-out is past half at half its time.
        let mid = 0.5;
        assert!(part_progress(mid, 0) > part_progress(mid, 1) && part_progress(mid, 1) > part_progress(mid, 3));
        assert!(part_progress(SLIDE_SECONDS / 2.0 / ENTER_SECONDS, 0) > 0.8);
        // Closing (t falling), the last order is out first.
        let t = 0.3;
        assert!(part_progress(t, 3) < part_progress(t, 0));
        // Monotonic.
        let mut last = 0.0;
        for k in 0..=100 {
            let p = part_progress(k as f32 / 100.0, 1);
            assert!(p >= last);
            last = p;
        }
    }

    /// A part starts fully past its edge of the window, wherever it sits.
    #[test]
    fn a_part_starts_past_its_edge() {
        let window = Vec2::new(1280.0, 720.0);
        let left = Rect::new(20.0, 74.0, 268.0, 650.0);
        let o = slide_offset(Edge::Left, left, window);
        assert!(left.max.x + o.x < 0.0 && o.y == 0.0);
        let right = Rect::new(968.0, 78.0, 1260.0, 650.0);
        assert!(right.min.x + slide_offset(Edge::Right, right, window).x > window.x);
        let top = Rect::new(22.0, 14.0, 400.0, 60.0);
        assert!(top.max.y + slide_offset(Edge::Top, top, window).y < 0.0);
        let bottom = Rect::new(22.0, 664.0, 1260.0, 706.0);
        assert!(bottom.min.y + slide_offset(Edge::Bottom, bottom, window).y > window.y);
        assert_eq!(slide_offset(Edge::Fade, bottom, window), Vec2::ZERO);
    }

    /// The fade writes colours at the alpha and gives them back exactly; a colour written meanwhile is kept.
    #[test]
    fn a_fade_gives_the_colours_back() {
        let mut world = World::new();
        let e = world.spawn((TextColor(Color::srgba(1.0, 0.5, 0.2, 0.8)), BackgroundColor(Color::srgb(0.1, 0.2, 0.3)))).id();
        let mut faded = HashMap::new();
        let mut run = |world: &mut World, faded: &mut HashMap<Entity, Faded>, a: f32| {
            let mut q = world.query::<(&mut TextColor, &mut BackgroundColor)>();
            let (t, b) = q.get_mut(world, e).unwrap();
            fade(faded, e, a, Some(t), Some(b), None, None);
        };
        run(&mut world, &mut faded, 0.5);
        assert!((world.get::<TextColor>(e).unwrap().0.alpha() - 0.4).abs() < 1e-5);
        assert!((world.get::<BackgroundColor>(e).unwrap().0.alpha() - 0.5).abs() < 1e-5);
        // A screen system writes a new text colour mid-fade: it becomes the base.
        world.get_mut::<TextColor>(e).unwrap().0 = Color::srgba(0.0, 1.0, 0.0, 1.0);
        run(&mut world, &mut faded, 0.25);
        assert!((world.get::<TextColor>(e).unwrap().0.alpha() - 0.25).abs() < 1e-5);
        run(&mut world, &mut faded, 1.0);
        assert_eq!(world.get::<TextColor>(e).unwrap().0, Color::srgba(0.0, 1.0, 0.0, 1.0));
        assert_eq!(world.get::<BackgroundColor>(e).unwrap().0, Color::srgb(0.1, 0.2, 0.3));
    }
}
