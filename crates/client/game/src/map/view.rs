//! The voyage map's view: zoom and pan, from the rim band's voyages out to the whole galaxy (Niklas, 14 Oct 2026:
//! "I would really like to be able to zoom in/out (even see the whole galaxy)").
//!
//! * **The view** ([`MapView`]) maps galaxy px (the stage px of the default view, where the content's map positions
//!   land) to stage px on screen: `screen = galaxy × zoom + offset`. The default (zoom 1, offset 0) is the map as it
//!   always was; entering the map starts there. Zoom runs from [`ZOOM_MAX`] in to the whole galaxy fitting
//!   [`VIEW_AREA`] (left of the panel, between the header and the legend). The pan keeps the galaxy (and
//!   [`ROOM`] around it) over the view area, and centres it once it is smaller than the area.
//! * **Easing**: the zoom eases towards its target (time constant [`ZOOM_TAU`], like the flight camera) around an
//!   anchor (the cursor, else the view area's centre) whose galaxy point stays under it all the way: the offset
//!   is worked out from the eased zoom every frame (`camera::CameraRig::zoom_around`'s fix), so the map, its
//!   labels and its markers never jump. A reset eases around the view's fixed point (the one point the current
//!   and the default view share), so it is the same kind of glide.
//! * **Input**: the wheel or a trackpad pinch zooms around the cursor, + / − around the centre, 0 or a double
//!   click on empty map space resets; a drag on empty map space (left), or anywhere on the map with the middle or
//!   right button, pans; on touch two fingers pinch and pan, one finger on empty space pans. The zoom buttons
//!   bottom right of the map (−, the zoom in %, +, reset) do the same; their tooltips name the keys.
//! * **Level of detail**: the voyages' labels and markers fade out as the view zooms out
//!   ([`detail_alpha`]); the galaxy's band names, the charted rim and the shipyard's beacon fade in
//!   ([`far_alpha`]). Labels keep their size; marker sizes stay in screen px (the detail mesh is rebuilt while
//!   the zoom changes). [`Anchored`] UI nodes are laid out for the default view and moved by a `UiTransform`.

use std::collections::BTreeMap;

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiTransform, Val2};
use bevy::window::PrimaryWindow;
use kit::ui_kit::{ScreenFx, UiClick, UiPointer, set_text};

use super::layout::{Band, PANEL_X, STAGE};
use super::{MapButton, MapRoot, Stage};

/// The closest zoom (the default is 1).
pub const ZOOM_MAX: f32 = 2.0;
/// The map area the view fits and keeps covered (stage px): left of the panel, between the header and the legend.
pub const VIEW_AREA: Rect = Rect { min: Vec2::new(0.0, 56.0), max: Vec2::new(PANEL_X, 664.0) };
/// Room kept around the galaxy's disc (galaxy px): what the pan may show past its edge, and the margin of the
/// whole-galaxy fit.
pub const ROOM: f32 = 120.0;
/// The zoom's easing time constant (s).
pub const ZOOM_TAU: f32 = 0.1;
/// Zoom steps: one wheel line, one key or button press.
const WHEEL_STEP: f32 = 1.15;
const KEY_STEP: f32 = 1.25;
/// Two clicks on empty space within this long (s) and this close (stage px) reset the view.
const DOUBLE_CLICK: (f64, f32) = (0.35, 8.0);
/// A press that moves less than this (stage px) before its release is a click, not a drag.
const CLICK_SLOP: f32 = 4.0;
/// How far inside the view area a voyage stepped to with the keys is brought (stage px).
const SHOW_INSET: f32 = 80.0;
/// The level-of-detail fades: voyage detail from fully out to fully in over this zoom range; the galaxy's names
/// and beacons the other way over theirs.
const DETAIL_FADE: (f32, f32) = (0.42, 0.72);
const FAR_FADE: (f32, f32) = (0.5, 0.8);

/// Zero slope at both ends.
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much of the voyages' detail (labels, markers) shows at a zoom: 1 from the default view in to
/// about half of it, 0 once the galaxy is most of the view.
pub fn detail_alpha(zoom: f32) -> f32 {
    smoothstep(DETAIL_FADE.0, DETAIL_FADE.1, zoom)
}

/// How much of the galaxy's own labels and beacons shows: the other way round, overlapping the detail's fade.
pub fn far_alpha(zoom: f32) -> f32 {
    1.0 - smoothstep(FAR_FADE.0, FAR_FADE.1, zoom)
}

/// The map's view (a resource): see the module doc.
#[derive(Resource, Clone, Debug)]
pub struct MapView {
    /// The zoom now (eased) and where it is going.
    pub zoom: f32,
    pub target: f32,
    /// Stage px on screen of the galaxy's origin.
    pub offset: Vec2,
    /// The galaxy's disc: centre and radius (galaxy px).
    disc: (Vec2, f32),
    /// While a zoom eases: the screen point (stage px) and the galaxy point kept under it.
    anchor: Option<(Vec2, Vec2)>,
    /// A pan easing to this offset (a reset at the default zoom, a voyage brought into view).
    glide: Option<Vec2>,
    /// A mouse or one-finger drag: the touch's id (none for the mouse), where it was last (stage px), and how far
    /// it has moved since the press.
    drag: Option<(Option<u64>, Vec2, f32)>,
    /// The last click on empty space: when and where.
    last_click: Option<(f64, Vec2)>,
    /// Two fingers: their distance and midpoint last frame (stage px).
    pinch: Option<(f32, Vec2)>,
}

impl MapView {
    /// The default view of a galaxy whose disc is (`centre`, `radius`).
    pub fn new(centre: Vec2, radius: f32) -> Self {
        Self { zoom: 1.0, target: 1.0, offset: Vec2::ZERO, disc: (centre, radius), anchor: None, glide: None, drag: None, last_click: None, pinch: None }
    }

    /// Back to the default view at once (entering the map).
    pub fn snap_default(&mut self) {
        *self = Self::new(self.disc.0, self.disc.1);
    }

    /// The view all the way out: the whole galaxy centred on the view area.
    pub fn whole(&self) -> Self {
        let mut v = Self::new(self.disc.0, self.disc.1);
        v.zoom = v.min_zoom();
        v.target = v.zoom;
        v.clamp();
        v
    }

    /// A galaxy point's place on screen (stage px).
    pub fn to_screen(&self, g: Vec2) -> Vec2 {
        g * self.zoom + self.offset
    }

    /// The galaxy point at a screen point (stage px).
    pub fn to_galaxy(&self, q: Vec2) -> Vec2 {
        (q - self.offset) / self.zoom
    }

    /// The farthest zoom: the galaxy and its [`ROOM`] fit the view area.
    pub fn min_zoom(&self) -> f32 {
        (VIEW_AREA.size() / (2.0 * (self.disc.1 + ROOM))).min_element().min(1.0)
    }

    /// Zooms the target by `factor` around a screen point (the view area's centre without one): that point's
    /// galaxy point stays under it while the zoom eases.
    pub fn zoom_around(&mut self, factor: f32, around: Option<Vec2>) {
        self.target = (self.target * factor).clamp(self.min_zoom(), ZOOM_MAX);
        let q = around.unwrap_or(VIEW_AREA.center());
        self.anchor = Some((q, self.to_galaxy(q)));
        self.glide = None;
    }

    /// Eases back to the default view: around the point the current view and the default one share, so
    /// everything glides straight to its default place; at the default zoom already, a plain pan.
    pub fn reset(&mut self) {
        self.target = 1.0;
        if (self.zoom - 1.0).abs() > 1e-3 {
            let q = self.offset / (1.0 - self.zoom);
            self.anchor = Some((q, q));
            self.glide = None;
        } else {
            self.anchor = None;
            self.glide = Some(Vec2::ZERO);
        }
    }

    /// Moves the map by `d` stage px (a drag): any easing pan or zoom anchor ends, the zoom eases on around the
    /// view area's centre.
    pub fn pan(&mut self, d: Vec2) {
        self.offset += d;
        self.glide = None;
        if self.anchor.is_some() {
            let q = VIEW_AREA.center();
            self.anchor = Some((q, self.to_galaxy(q)));
        }
        self.clamp();
    }

    /// Two fingers moved: zoom by `factor` around the old midpoint, which follows the fingers to `mid`.
    pub fn pinch(&mut self, factor: f32, last_mid: Vec2, mid: Vec2) {
        let g = self.to_galaxy(last_mid);
        self.zoom = (self.zoom * factor).clamp(self.min_zoom(), ZOOM_MAX);
        self.target = self.zoom;
        self.offset = mid - g * self.zoom;
        self.anchor = None;
        self.glide = None;
        self.clamp();
    }

    /// A voyage stepped to with the keys: when it lies outside the view area (less [`SHOW_INSET`]), the view pans
    /// until it doesn't.
    pub fn show(&mut self, g: Vec2) {
        let inner = Rect { min: VIEW_AREA.min + SHOW_INSET, max: VIEW_AREA.max - SHOW_INSET };
        let q = self.to_screen(g);
        let d = q.clamp(inner.min, inner.max) - q;
        if d.length() > 0.5 {
            self.glide = Some(self.offset + d);
        }
    }

    /// A click (or a tap) on empty map space at `at` (stage px) at `now` (s): is it the second of a double click
    /// (two within [`DOUBLE_CLICK`])?
    pub fn empty_click(&mut self, now: f64, at: Vec2) -> bool {
        match self.last_click {
            Some((t, p)) if now - t < DOUBLE_CLICK.0 && p.distance(at) < DOUBLE_CLICK.1 => {
                self.last_click = None;
                true
            }
            _ => {
                self.last_click = Some((now, at));
                false
            }
        }
    }

    /// A voyage clicked while the view is zoomed out past its labels: zoom in to the default zoom around it.
    pub fn zoom_to(&mut self, g: Vec2) {
        self.zoom_around(1.0 / self.target, Some(self.to_screen(g)));
    }

    /// One frame: the zoom eases towards its target, the offset follows the anchor (or the pan glides), then the
    /// galaxy is kept over the view area.
    pub fn ease(&mut self, dt: f32) {
        let k = 1.0 - (-dt / ZOOM_TAU).exp();
        let z = self.zoom + (self.target - self.zoom) * k;
        // Within 0.1 % the zoom has arrived (an exponential never does).
        let arrived = (z - self.target).abs() <= 1e-3 * self.target;
        self.zoom = if arrived { self.target } else { z };
        if let Some((q, g)) = self.anchor {
            self.offset = q - g * self.zoom;
            if arrived {
                self.anchor = None;
            }
        }
        if let Some(to) = self.glide {
            self.offset = self.offset.lerp(to, k);
            if self.offset.distance(to) < 0.05 {
                self.offset = to;
                self.glide = None;
            }
        }
        // The default view is exact once a reset arrives.
        if self.zoom == 1.0 && self.target == 1.0 && self.anchor.is_none() && self.glide.is_none() && self.offset.length() < 1e-3 {
            self.offset = Vec2::ZERO;
        }
        self.clamp();
    }

    /// Keeps the galaxy's disc and its [`ROOM`] over the view area on each axis; one smaller than the area is
    /// centred on it.
    fn clamp(&mut self) {
        let (c, r) = (self.disc.0, self.disc.1 + ROOM);
        let (lo, hi) = ((c - r) * self.zoom, (c + r) * self.zoom);
        let (a, b) = (VIEW_AREA.min, VIEW_AREA.max);
        for i in 0..2 {
            let o = if hi[i] - lo[i] <= b[i] - a[i] { (a[i] + b[i] - lo[i] - hi[i]) / 2.0 } else { self.offset[i].clamp(b[i] - hi[i], a[i] - lo[i]) };
            self.offset[i] = o;
        }
    }

    /// Where the top band names sit (screen stage x on the line [`TOP_Y`]): this band's halfway between where its
    /// edges cross the line (right of the header), the next band's just past the gate, if those are on the map.
    pub fn top_band_x(&self, band: &Band) -> [Option<f32>; 2] {
        let b = Band { centre: self.to_screen(band.centre), outer: band.outer * self.zoom, inner: band.inner * self.zoom, ..band.clone() };
        let inner = b.x_at(b.inner, TOP_Y);
        let this = match (b.x_at(b.outer, TOP_Y), inner) {
            (Some(o), Some(i)) => Some(((o.max(0.0) + i) / 2.0).max(330.0)),
            (Some(o), None) => Some(((o.max(0.0) + PANEL_X) / 2.0).max(330.0)),
            (None, _) => None,
        };
        let next = inner.filter(|_| band.next_key.is_some()).map(|i| i + 34.0);
        [this.filter(|x| *x < PANEL_X - 30.0), next.filter(|x| *x + 26.0 < PANEL_X)]
    }
}

/// The line the top band names sit on (stage y).
pub const TOP_Y: f32 = 30.0;

/// The view's buttons (bottom right of the map).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewButton {
    ZoomIn,
    ZoomOut,
    Reset,
}

/// What a map label or click area follows, and the layer it fades with.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum Anchored {
    /// A galaxy point (where it was laid out, at the default view).
    At(Vec2, Layer),
    /// A galaxy band's name at a galaxy point, laid out where the whole-galaxy view ([`MapView::whole`]) puts it
    /// (inside the map, as nodes laid out past its clipping edges don't draw); fades with [`Layer::Far`].
    Far(Vec2),
    /// A top band name: 0 this band, 1 the next (laid out at `x`).
    TopBand(usize, f32),
}

/// The layer a map element fades with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Always shown.
    Always,
    /// The voyages' detail ([`detail_alpha`]).
    Detail,
    /// The galaxy's own names and beacons ([`far_alpha`]).
    Far,
}

impl Layer {
    pub fn alpha(self, zoom: f32) -> f32 {
        match self {
            Layer::Always => 1.0,
            Layer::Detail => detail_alpha(zoom),
            Layer::Far => far_alpha(zoom),
        }
    }
}

/// The zoom readout between − and +.
#[derive(Component)]
pub struct ZoomText;

/// Wheel, pinch, keys, drags, double clicks and the view's buttons.
#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    stage: Single<(&ComputedNode, &UiGlobalTransform), With<Stage>>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    pointer: Res<UiPointer>,
    mut clicks: MessageReader<UiClick>,
    map_buttons: Query<&MapButton>,
    mut view: ResMut<MapView>,
) {
    for c in clicks.read() {
        match map_buttons.get(c.0) {
            Ok(MapButton::View(ViewButton::ZoomIn)) => view.zoom_around(KEY_STEP, None),
            Ok(MapButton::View(ViewButton::ZoomOut)) => view.zoom_around(1.0 / KEY_STEP, None),
            Ok(MapButton::View(ViewButton::Reset)) => view.reset(),
            _ => {}
        }
    }
    if !pointer.typing {
        if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
            view.zoom_around(KEY_STEP, None);
        }
        if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
            view.zoom_around(1.0 / KEY_STEP, None);
        }
        if keys.any_just_pressed([KeyCode::Digit0, KeyCode::Numpad0]) {
            view.reset();
        }
    }
    // Window px (logical) → stage px.
    let (node, tf) = *stage;
    let scale = window.scale_factor();
    let k = node.size.x / scale / STAGE.x;
    if k <= 0.0 {
        return;
    }
    let centre = tf.translation / scale;
    let to_stage = |c: Vec2| (c - centre) / k + STAGE / 2.0;
    let on_map = |q: Vec2| q.x < PANEL_X;
    let cursor = window.cursor_position().map(to_stage);

    // The wheel (a trackpad pinch arrives as one) over the map, labels included.
    if scroll.delta.y != 0.0
        && let Some(c) = cursor.filter(|c| on_map(*c))
    {
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
        };
        view.zoom_around(WHEEL_STEP.powf(steps.clamp(-5.0, 5.0)), Some(c));
    }

    // Two fingers: pinch and pan.
    let mut fingers = touches.iter();
    if let (Some(a), Some(b)) = (fingers.next(), fingers.next()) {
        let (pa, pb) = (to_stage(a.position()), to_stage(b.position()));
        let (dist, mid) = (pa.distance(pb).max(1.0), (pa + pb) / 2.0);
        if let Some((last_dist, last_mid)) = view.pinch {
            view.pinch(dist / last_dist, last_mid, mid);
        }
        view.pinch = Some((dist, mid));
        view.drag = None;
        return;
    }
    view.pinch = None;

    // Drags: the left button or one finger from empty map space; the middle or right button anywhere on the map.
    let now = time.elapsed_secs_f64();
    if view.drag.is_none() {
        let mouse_start = buttons.just_pressed(MouseButton::Left) && !pointer.blocked || buttons.any_just_pressed([MouseButton::Middle, MouseButton::Right]);
        if mouse_start && let Some(c) = cursor.filter(|c| on_map(*c)) {
            view.drag = Some((None, c, 0.0));
        } else if let Some(t) = touches.iter_just_pressed().next()
            && !pointer.blocked
            && on_map(to_stage(t.position()))
        {
            view.drag = Some((Some(t.id()), to_stage(t.position()), 0.0));
        }
    }
    let Some((id, last, moved)) = view.drag else { return };
    let held = match id {
        None => cursor.filter(|_| buttons.any_pressed([MouseButton::Left, MouseButton::Middle, MouseButton::Right])),
        Some(id) => touches.get_pressed(id).map(|t| to_stage(t.position())),
    };
    match held {
        Some(at) => {
            let d = at - last;
            if d != Vec2::ZERO {
                view.pan(d);
            }
            view.drag = Some((id, at, moved + d.length()));
        }
        None => {
            view.drag = None;
            // A left click (or a tap) on empty space: two of them reset the view.
            let left = id.is_some() || buttons.just_released(MouseButton::Left);
            if left && moved < CLICK_SLOP && view.empty_click(now, last) {
                view.reset();
            }
        }
    }
}

/// One frame of the view's easing.
pub(super) fn ease(time: Res<Time<Real>>, mut view: ResMut<MapView>) {
    let dt = time.delta_secs().min(0.1);
    // Only an easing view is written (the meshes and labels follow changes).
    if view.anchor.is_some() || view.glide.is_some() || view.zoom != view.target {
        view.ease(dt);
    }
}

/// Moves the anchored labels and click areas with the view, fades them with their layer (hidden when gone), and
/// writes the zoom readout. Every frame (a few dozen nodes), writing only what changes.
///
/// Colours are faded from what they are without the fade, as `ui_kit`'s screen transition does: a colour someone
/// else writes meanwhile (`sync_map`) becomes the one faded. Only while the screen is open and settled, so the two
/// fades never write over each other.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn place_labels(
    view: Res<MapView>,
    geo: Res<super::MapGeometry>,
    root: Single<&ScreenFx, With<MapRoot>>,
    mut anchored: Query<(Entity, &Anchored, &mut UiTransform, &mut Visibility)>,
    children: Query<&Children>,
    mut colours: Query<(Option<&mut TextColor>, Option<&mut BackgroundColor>)>,
    mut readout: Query<&mut Text, With<ZoomText>>,
    mut faded: Local<BTreeMap<(Entity, bool), (Color, Color)>>,
) {
    let settled = root.is_open() && root.alpha() >= 1.0;
    let top = view.top_band_x(&geo.0.band);
    let whole = view.whole();
    for (e, a, mut tf, mut vis) in &mut anchored {
        let (d, layer) = match *a {
            Anchored::At(g, layer) => (view.to_screen(g) - g, layer),
            Anchored::Far(g) => (view.to_screen(g) - whole.to_screen(g), Layer::Far),
            Anchored::TopBand(k, x) => match top[k] {
                Some(at) => (Vec2::new(at - x, 0.0), Layer::Detail),
                None => (Vec2::ZERO, Layer::Detail),
            },
        };
        let gone = matches!(*a, Anchored::TopBand(k, _) if top[k].is_none());
        let alpha = if gone { 0.0 } else { layer.alpha(view.zoom) };
        let want = UiTransform { translation: Val2::px(d.x, d.y), ..*tf };
        if *tf != want {
            *tf = want;
        }
        let v = if alpha > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != v {
            *vis = v;
        }
        if !settled || layer == Layer::Always {
            continue;
        }
        let mut stack = vec![e];
        while let Some(n) = stack.pop() {
            if let Ok((text, bg)) = colours.get_mut(n) {
                if let Some(mut t) = text {
                    fade(&mut faded, (n, false), &mut t.0, alpha);
                }
                if let Some(mut b) = bg {
                    fade(&mut faded, (n, true), &mut b.0, alpha);
                }
            }
            stack.extend(children.get(n).into_iter().flatten());
        }
    }
    for mut text in &mut readout {
        set_text(&mut text, &format!("{:.0} %", view.zoom * 100.0));
    }
}

/// Writes `colour` at `alpha` of its unfaded value (remembered; a value written by someone else since the last
/// fade becomes the new unfaded one). Each colour of a node (its text, its background) has its own `key`. Written
/// only when it changes.
fn fade(faded: &mut BTreeMap<(Entity, bool), (Color, Color)>, key: (Entity, bool), colour: &mut Color, alpha: f32) {
    let entry = faded.entry(key).or_insert((*colour, *colour));
    if *colour != entry.1 {
        entry.0 = *colour;
    }
    let want = entry.0.with_alpha(entry.0.alpha() * alpha);
    if *colour != want {
        *colour = want;
    }
    entry.1 = want;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rim band's galaxy as the base content lays it out: centre (1500, 400), radius about 1414.
    fn view() -> MapView {
        MapView::new(Vec2::new(1500.0, 400.0), 1414.0)
    }

    /// A wheel zoom around the cursor keeps the galaxy point under it in place on every frame of the ease, not
    /// only once it arrives, at 30 and at 144 fps; it arrives at the target.
    #[test]
    fn a_zoom_keeps_its_anchor_still_all_the_way() {
        for fps in [30.0, 144.0] {
            let mut v = view();
            let cursor = Vec2::new(400.0, 300.0);
            let g = v.to_galaxy(cursor);
            v.zoom_around(1.5, Some(cursor));
            for _ in 0..(fps as usize) {
                v.ease(1.0 / fps);
                assert!(v.to_screen(g).distance(cursor) < 1e-3, "{fps}: {} vs {cursor}", v.to_screen(g));
            }
            assert_eq!(v.zoom, 1.5);
        }
    }

    /// The zoom stays between the whole galaxy fitting the view area and [`ZOOM_MAX`]; fully out the galaxy is
    /// centred on the view area whatever was under the cursor.
    #[test]
    fn the_zoom_is_limited_and_the_whole_galaxy_fits_fully_out() {
        let mut v = view();
        v.zoom_around(10.0, None);
        assert_eq!(v.target, ZOOM_MAX);
        v.zoom_around(1e-3, Some(Vec2::new(50.0, 600.0)));
        let min = v.min_zoom();
        assert_eq!(v.target, min);
        assert!((1414.0 + ROOM) * 2.0 * min <= VIEW_AREA.height() + 1e-3 && min > 0.15, "{min}");
        for _ in 0..120 {
            v.ease(1.0 / 60.0);
        }
        assert_eq!(v.zoom, min);
        assert!(v.to_screen(Vec2::new(1500.0, 400.0)).distance(VIEW_AREA.center()) < 1e-3);
    }

    /// The default view stays put under the pan bounds; a pan stops at the galaxy's edge plus [`ROOM`].
    #[test]
    fn the_pan_keeps_the_galaxy_over_the_map() {
        let mut v = view();
        v.ease(1.0 / 60.0);
        assert_eq!((v.zoom, v.offset), (1.0, Vec2::ZERO));
        v.pan(Vec2::new(5000.0, 0.0));
        // The galaxy's left edge (and its room) at the view area's left edge.
        assert!(((1500.0 - 1414.0 - ROOM) + v.offset.x - VIEW_AREA.min.x).abs() < 1e-3, "{}", v.offset);
        v.pan(Vec2::new(-1e5, -1e5));
        assert!(((1500.0 + 1414.0 + ROOM) + v.offset.x - VIEW_AREA.max.x).abs() < 1e-3, "{}", v.offset);
        assert!(((400.0 + 1414.0 + ROOM) + v.offset.y - VIEW_AREA.max.y).abs() < 1e-3, "{}", v.offset);
    }

    /// A reset from a zoomed, panned view glides back to the exact default view along one similarity (the fixed
    /// point stays still, no frame jumps); at the default zoom it pans back.
    #[test]
    fn a_reset_glides_back_to_the_default_view() {
        let mut v = view();
        v.zoom_around(1.8, Some(Vec2::new(600.0, 200.0)));
        for _ in 0..60 {
            v.ease(1.0 / 60.0);
        }
        v.pan(Vec2::new(-40.0, 25.0));
        let fixed = v.offset / (1.0 - v.zoom);
        let g = v.to_galaxy(fixed);
        v.reset();
        let mut last = v.to_screen(Vec2::new(300.0, 400.0));
        for _ in 0..90 {
            v.ease(1.0 / 60.0);
            assert!(v.to_screen(g).distance(fixed) < 1e-2);
            let now = v.to_screen(Vec2::new(300.0, 400.0));
            assert!(now.distance(last) < 60.0, "a jump of {}", now.distance(last));
            last = now;
        }
        assert_eq!((v.zoom, v.target), (1.0, 1.0), "{v:?}");
        assert_eq!(v.offset, Vec2::ZERO);
        v.pan(Vec2::new(30.0, 30.0));
        v.reset();
        for _ in 0..120 {
            v.ease(1.0 / 60.0);
        }
        assert_eq!((v.zoom, v.offset), (1.0, Vec2::ZERO));
    }

    /// The voyages' detail shows from the default view in and is gone fully out; the galaxy's names the other way;
    /// both fades overlap, so something is always readable.
    #[test]
    fn detail_fades_out_and_the_galaxy_names_in_as_the_view_zooms_out() {
        let min = view().min_zoom();
        assert_eq!((detail_alpha(1.0), far_alpha(1.0)), (1.0, 0.0));
        assert_eq!((detail_alpha(ZOOM_MAX), far_alpha(ZOOM_MAX)), (1.0, 0.0));
        assert_eq!((detail_alpha(min), far_alpha(min)), (0.0, 1.0));
        let mut z = min;
        let (mut d, mut f) = (0.0, 1.0);
        while z <= 1.0 {
            let (dz, fz) = (detail_alpha(z), far_alpha(z));
            assert!(dz >= d && fz <= f && dz + fz >= 0.5, "{z}: {dz} {fz}");
            (d, f) = (dz, fz);
            z += 0.01;
        }
        assert_eq!(Layer::Always.alpha(min), 1.0);
    }

    /// A label faded out and back in gets its colours back exactly, its text and its background each their own (a
    /// node has both); a colour written meanwhile (a hover) becomes the one faded.
    #[test]
    fn the_label_fade_gives_each_colour_back() {
        let mut faded = BTreeMap::new();
        let e = Entity::from_raw_u32(7).unwrap();
        let (mut text, mut bg) = (Color::srgba(0.5, 0.9, 0.8, 0.95), Color::NONE);
        for alpha in [0.0, 0.3, 1.0] {
            fade(&mut faded, (e, false), &mut text, alpha);
            fade(&mut faded, (e, true), &mut bg, alpha);
        }
        assert_eq!((text, bg), (Color::srgba(0.5, 0.9, 0.8, 0.95), Color::NONE));
        fade(&mut faded, (e, false), &mut text, 0.5);
        text = Color::WHITE;
        fade(&mut faded, (e, false), &mut text, 0.5);
        assert_eq!(text, Color::WHITE.with_alpha(0.5));
        fade(&mut faded, (e, false), &mut text, 1.0);
        assert_eq!(text, Color::WHITE);
    }

    /// Two quick clicks close together are a double click; a slow or a distant second one starts over, and a third
    /// quick one is a first again.
    #[test]
    fn a_double_click_is_two_quick_close_clicks() {
        let mut v = view();
        let at = Vec2::new(300.0, 300.0);
        assert!(!v.empty_click(1.0, at));
        assert!(v.empty_click(1.2, at + Vec2::splat(3.0)));
        assert!(!v.empty_click(1.3, at));
        assert!(!v.empty_click(2.0, at), "too slow");
        assert!(!v.empty_click(2.1, at + Vec2::new(40.0, 0.0)), "too far");
        assert!(v.empty_click(2.2, at + Vec2::new(40.0, 0.0)));
    }

    /// A voyage stepped to off the view comes into it; one in view leaves the view alone.
    #[test]
    fn stepping_to_a_voyage_off_the_view_brings_it_in() {
        let mut v = view();
        v.zoom_around(2.0, Some(Vec2::new(100.0, 600.0)));
        for _ in 0..60 {
            v.ease(1.0 / 60.0);
        }
        let far = Vec2::new(700.0, 260.0);
        assert!(!VIEW_AREA.contains(v.to_screen(far)));
        v.show(far);
        for _ in 0..90 {
            v.ease(1.0 / 60.0);
        }
        let q = v.to_screen(far);
        assert!(VIEW_AREA.contains(q), "{q}");
        let at = v.offset;
        v.show(far);
        assert!(v.glide.is_none() && v.offset == at);
    }

    /// The top band names sit where they always did at the default view and slide with the arcs.
    #[test]
    fn the_top_band_names_follow_their_arcs() {
        let content = data::base_content();
        let geo = super::super::layout::Geometry::new(&content);
        let mut v = view();
        let [this, next] = v.top_band_x(&geo.band);
        let b = &geo.band;
        let (outer, inner) = (b.x_at(b.outer, TOP_Y).unwrap_or(0.0).max(0.0), b.x_at(b.inner, TOP_Y).unwrap());
        assert_eq!(this, Some(((outer + inner) / 2.0).max(330.0)));
        assert_eq!(next, Some(inner + 34.0));
        v.pan(Vec2::new(-200.0, 0.0));
        assert_eq!(v.top_band_x(&geo.band)[1], Some(inner + 34.0 - 200.0));
    }
}
