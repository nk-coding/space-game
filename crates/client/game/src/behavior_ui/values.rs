//! The inspector's numbers as plain data: what kind a number is (metres, a share of the weapon reach, a
//! percentage, seconds, a count), its range, the −/+ steps and the slider's scale, and the metres ↔ % switch of
//! distances. Pure and tested; `inspect.rs` builds the controls from it, `screen.rs` draws them.
//!
//! * **Steps** are round numbers that grow with the value (5 m below 50 m, 10 m below 200 m, ...), and a step
//!   lands on the next multiple (`67%` + → `70%`), so values stay tidy.
//! * **The slider** is soft-logarithmic for distances and times (most behaviors use 30–300 m of a 0–5000 m
//!   range, or 2–10 s of 600 s), linear for percentages and counts; it snaps to the step at the value.

use data::behavior::{MAX_REACH_PERCENT, Unit, Value};
use data::{NodeParam, ParamType, Quantity};

/// What a number means, which decides its unit, its steps and its slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Metres,
    /// A distance as a share of the ship's weapon reach (`67%`).
    Reach,
    Percent,
    Seconds,
    Count,
}

/// Where a count's slider ends when nothing else says (enemies near).
const COUNT_HI: f32 = 20.0;
/// Where a time threshold's slider ends when the node has no period.
const SECONDS_HI: f32 = 60.0;

impl Kind {
    /// The kind of a number of a quantity as written (`%` on a distance is a share of the weapon reach).
    pub fn of(q: Quantity, unit: Option<Unit>) -> Kind {
        match (q, unit) {
            (Quantity::Distance, Some(Unit::Percent)) => Kind::Reach,
            (Quantity::Distance, _) => Kind::Metres,
            (Quantity::Percent, _) => Kind::Percent,
            (Quantity::Seconds, _) => Kind::Seconds,
            (Quantity::Count, _) => Kind::Count,
        }
    }

    /// The unit the editor writes (metres and seconds with their unit, so the text says what it means).
    pub fn unit(self) -> Option<Unit> {
        match self {
            Kind::Metres => Some(Unit::Metres),
            Kind::Reach | Kind::Percent => Some(Unit::Percent),
            Kind::Seconds => Some(Unit::Seconds),
            Kind::Count => None,
        }
    }

    /// `x` as a value of this kind.
    pub fn value(self, x: f32) -> Value {
        Value::Number(x, self.unit())
    }

    /// The −/+ step at `x`: round numbers that grow with the value.
    pub fn step(self, x: f32) -> f32 {
        match self {
            Kind::Metres if x < 50.0 => 5.0,
            Kind::Metres if x < 200.0 => 10.0,
            Kind::Metres if x < 1000.0 => 50.0,
            Kind::Metres => 100.0,
            Kind::Reach if x < 200.0 => 5.0,
            Kind::Reach if x < 500.0 => 10.0,
            Kind::Reach => 50.0,
            Kind::Percent => 5.0,
            Kind::Seconds if x < 10.0 => 0.5,
            Kind::Seconds if x < 60.0 => 1.0,
            Kind::Seconds if x < 300.0 => 5.0,
            Kind::Seconds => 30.0,
            Kind::Count => 1.0,
        }
    }

    /// The slider's softness: `Some(k)` for a soft-logarithmic slider (`ln(1 + x / k)`), `None` for linear.
    fn soft(self) -> Option<f32> {
        match self {
            Kind::Metres | Kind::Reach => Some(20.0),
            Kind::Seconds => Some(2.0),
            Kind::Percent | Kind::Count => None,
        }
    }

    /// The unit after a range (` M`, ` %`, ` S`, nothing for counts).
    pub fn unit_word(self) -> &'static str {
        match self {
            Kind::Metres => " M",
            Kind::Reach | Kind::Percent => " %",
            Kind::Seconds => " S",
            Kind::Count => "",
        }
    }
}

/// A number's range and how the controls move it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub kind: Kind,
    pub lo: f32,
    pub hi: f32,
}

impl Scale {
    /// The scale of a numeric parameter for its value as written: 0–1000 % for a share of the weapon reach,
    /// else the parameter's `min` and `max` (`None` for a choice or a word).
    pub fn param(p: &NodeParam, value: &Value) -> Option<Scale> {
        let q = p.ty.quantity()?;
        let Value::Number(x, unit) = value else { return None };
        let kind = Kind::of(q, *unit);
        let (lo, hi) = param_range(p, kind);
        Some(Scale { kind, lo, hi: hi.max(*x).max(lo) })
    }

    /// The scale of a value condition's threshold: from 0 to 100 % for percentages, to `period` (the node's
    /// own time, e.g. `cycle(9s)`) or 60 s for times, to 20 for counts; wider if the threshold already is.
    pub fn threshold(q: Quantity, value: &Value, period: Option<f32>) -> Option<Scale> {
        let Value::Number(x, unit) = value else { return None };
        let kind = Kind::of(q, *unit);
        let hi = match kind {
            Kind::Seconds => period.unwrap_or(SECONDS_HI),
            k => default_hi(k),
        };
        Some(Scale { kind, lo: 0.0, hi: hi.max(*x) })
    }

    /// Where `x` sits on the slider, 0..1.
    pub fn share(&self, x: f32) -> f32 {
        let span = self.hi - self.lo;
        if span <= 0.0 {
            return 0.0;
        }
        let t = ((x - self.lo) / span).clamp(0.0, 1.0);
        match self.kind.soft() {
            Some(k) => (1.0 + t * span / k).ln() / (1.0 + span / k).ln(),
            None => t,
        }
    }

    /// The value at a slider position (0..1), snapped to the step there and kept in range.
    pub fn at(&self, share: f32) -> f32 {
        let span = self.hi - self.lo;
        let s = share.clamp(0.0, 1.0);
        let raw = match self.kind.soft() {
            Some(k) => self.lo + k * (((1.0 + span / k).ln() * s).exp() - 1.0),
            None => self.lo + s * span,
        };
        let step = self.kind.step(raw);
        ((raw / step).round() * step).clamp(self.lo, self.hi)
    }

    /// One step up from `x` to the next multiple of the step (`None` at the top).
    pub fn up(&self, x: f32) -> Option<f32> {
        let step = self.kind.step(x);
        let next = (((x / step) + 1e-3).floor() * step + step).min(self.hi);
        (next > x + 1e-4).then_some(next)
    }

    /// One step down from `x` to the multiple of the step below it (`None` at the bottom).
    pub fn down(&self, x: f32) -> Option<f32> {
        let step = self.kind.step((x - 1e-3).max(0.0));
        let next = (((x / step) - 1e-3).ceil() * step - step).max(self.lo);
        (next < x - 1e-4).then_some(next)
    }

    /// The range as the inspector prints it (`0 – 100 %`, `1 – 600 S`).
    pub fn label(&self) -> String {
        format!("{} – {}{}", self.lo, self.hi, self.kind.unit_word())
    }
}

/// A parameter's range for numbers of a kind: 0–1000 % for a share of the weapon reach, else `min` and `max`.
fn param_range(p: &NodeParam, kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Reach => (0.0, MAX_REACH_PERCENT),
        _ => (p.min.unwrap_or(0.0), p.max.unwrap_or_else(|| default_hi(kind))),
    }
}

fn default_hi(kind: Kind) -> f32 {
    match kind {
        Kind::Metres => 5000.0,
        Kind::Reach => MAX_REACH_PERCENT,
        Kind::Percent => 100.0,
        Kind::Seconds => 600.0,
        Kind::Count => COUNT_HI,
    }
}

/// A distance parameter's value in the other unit: metres ↔ a share of the weapon reach. With the design's
/// weapon reach known (metres, above 0) the distance stays the same, rounded to a step; without it the number
/// stays and only the unit changes. Kept in the parameter's range. `None` if `value` is not a distance number.
pub fn switch_unit(p: &NodeParam, value: &Value, reach: Option<f32>) -> Option<Value> {
    if p.ty != ParamType::Distance {
        return None;
    }
    let Value::Number(x, unit) = value else { return None };
    let from = Kind::of(Quantity::Distance, *unit);
    let to = if from == Kind::Reach { Kind::Metres } else { Kind::Reach };
    let y = match reach.filter(|r| *r > 0.0) {
        Some(r) => {
            let y = if to == Kind::Metres { x / 100.0 * r } else { x / r * 100.0 };
            let step = to.step(y);
            (y / step).round() * step
        }
        None => *x,
    };
    let (lo, hi) = param_range(p, to);
    Some(to.value(y.clamp(lo, hi)))
}

/// The metres a share of the weapon reach stands for on this design (`≈ 100 M`), when the reach is known.
pub fn reach_hint(value: &Value, reach: Option<f32>) -> Option<String> {
    let (Value::Number(x, Some(Unit::Percent)), Some(r)) = (value, reach.filter(|r| *r > 0.0)) else { return None };
    Some(format!("≈ {} M", (x / 100.0 * r).round()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(content: &data::Content, node: &str, k: usize) -> NodeParam {
        content.behavior_nodes.get(node).unwrap().params[k].clone()
    }

    #[test]
    fn steps_land_on_round_numbers_within_the_range() {
        let pct = Scale { kind: Kind::Percent, lo: 0.0, hi: 100.0 };
        assert_eq!((pct.down(25.0), pct.up(25.0)), (Some(20.0), Some(30.0)));
        assert_eq!((pct.down(27.0), pct.up(27.0)), (Some(25.0), Some(30.0)));
        assert_eq!((pct.down(0.0), pct.up(100.0)), (None, None));
        assert_eq!(pct.up(98.0), Some(100.0));
        // A share of the weapon reach: 5 % steps below 200 %, then 10 %.
        let reach = Scale { kind: Kind::Reach, lo: 0.0, hi: 1000.0 };
        assert_eq!((reach.down(67.0), reach.up(67.0)), (Some(65.0), Some(70.0)));
        assert_eq!((reach.down(200.0), reach.up(200.0)), (Some(195.0), Some(210.0)));
        // Metres grow their step; the orbit radius stops at its minimum of 1 m.
        let m = Scale { kind: Kind::Metres, lo: 1.0, hi: 5000.0 };
        assert_eq!((m.down(45.0), m.up(45.0)), (Some(40.0), Some(50.0)));
        assert_eq!((m.down(50.0), m.up(50.0)), (Some(45.0), Some(60.0)));
        assert_eq!((m.down(5.0), m.down(1.0)), (Some(1.0), None));
        assert_eq!(m.up(4990.0), Some(5000.0));
        // Half seconds for short times, whole counts.
        let s = Scale { kind: Kind::Seconds, lo: 0.0, hi: 60.0 };
        assert_eq!((s.down(2.0), s.up(2.0)), (Some(1.5), Some(2.5)));
        assert_eq!((s.down(10.0), s.up(10.0)), (Some(9.5), Some(11.0)));
        let c = Scale { kind: Kind::Count, lo: 0.0, hi: 20.0 };
        assert_eq!((c.down(3.0), c.up(3.0)), (Some(2.0), Some(4.0)));
    }

    #[test]
    fn the_slider_round_trips_and_snaps() {
        let m = Scale { kind: Kind::Metres, lo: 0.0, hi: 5000.0 };
        // Soft-logarithmic: everyday distances get most of the track.
        assert!(m.share(100.0) > 0.3 && m.share(300.0) > 0.45 && m.share(1000.0) < 0.8, "{}", m.share(100.0));
        assert_eq!((m.share(0.0), m.share(5000.0)), (0.0, 1.0));
        for x in [0.0, 45.0, 100.0, 150.0, 300.0, 1000.0, 5000.0] {
            assert_eq!(m.at(m.share(x)), x, "{x}");
        }
        assert_eq!(m.at(m.share(47.0)), 45.0, "snapped to 5 m");
        // Linear for percentages; out of range clamps.
        let pct = Scale { kind: Kind::Percent, lo: 0.0, hi: 100.0 };
        assert_eq!((pct.share(25.0), pct.at(0.25), pct.at(0.26)), (0.25, 25.0, 25.0));
        assert_eq!((pct.at(-1.0), pct.at(2.0)), (0.0, 100.0));
        let s = Scale { kind: Kind::Seconds, lo: 1.0, hi: 600.0 };
        assert_eq!(s.at(s.share(9.0)), 9.0);
        assert_eq!(s.at(0.0), 1.0);
    }

    #[test]
    fn scales_come_from_the_content_ranges() {
        let content = data::base_content();
        let radius = param(&content, "orbit", 0);
        assert_eq!(Scale::param(&radius, &Value::Number(45.0, Some(Unit::Metres))), Some(Scale { kind: Kind::Metres, lo: 1.0, hi: 5000.0 }));
        assert_eq!(Scale::param(&radius, &Value::Number(80.0, Some(Unit::Percent))), Some(Scale { kind: Kind::Reach, lo: 0.0, hi: 1000.0 }));
        // A bare number on a distance is metres.
        assert_eq!(Scale::param(&radius, &Value::Number(45.0, None)).unwrap().kind, Kind::Metres);
        let period = param(&content, "cycle", 0);
        assert_eq!(Scale::param(&period, &Value::Number(9.0, Some(Unit::Seconds))), Some(Scale { kind: Kind::Seconds, lo: 1.0, hi: 600.0 }));
        assert_eq!(Scale::param(&param(&content, "orbit", 1), &Value::Word("cw".into())), None);
        // Thresholds: percentages to 100 %, a cycle's time to its period, counts to 20 (or the value).
        let pct = Value::Number(25.0, Some(Unit::Percent));
        assert_eq!(Scale::threshold(Quantity::Percent, &pct, None).unwrap().label(), "0 – 100 %");
        assert_eq!(Scale::threshold(Quantity::Seconds, &Value::Number(5.0, Some(Unit::Seconds)), Some(9.0)).unwrap().hi, 9.0);
        assert_eq!(Scale::threshold(Quantity::Seconds, &Value::Number(5.0, None), None).unwrap().hi, 60.0);
        assert_eq!(Scale::threshold(Quantity::Count, &Value::Number(30.0, None), None).unwrap().hi, 30.0);
    }

    #[test]
    fn distances_switch_between_metres_and_reach() {
        let content = data::base_content();
        let range = param(&content, "approach", 0);
        let pct = Value::Number(67.0, Some(Unit::Percent));
        // With the design's reach (150 m): the same distance, rounded to a step.
        assert_eq!(switch_unit(&range, &pct, Some(150.0)), Some(Value::Number(100.0, Some(Unit::Metres))));
        assert_eq!(switch_unit(&range, &Value::Number(100.0, Some(Unit::Metres)), Some(150.0)), Some(Value::Number(65.0, Some(Unit::Percent))));
        // Without it the number stays; a bare number counts as metres.
        assert_eq!(switch_unit(&range, &pct, None), Some(Value::Number(67.0, Some(Unit::Metres))));
        assert_eq!(switch_unit(&range, &Value::Number(45.0, None), Some(0.0)), Some(Value::Number(45.0, Some(Unit::Percent))));
        // Kept in range: 4000 m at a 100 m reach would be 4000 %.
        assert_eq!(switch_unit(&range, &Value::Number(4000.0, Some(Unit::Metres)), Some(100.0)), Some(Value::Number(1000.0, Some(Unit::Percent))));
        // Only distances switch.
        assert_eq!(switch_unit(&param(&content, "cycle", 0), &Value::Number(9.0, None), Some(150.0)), None);
        assert_eq!(reach_hint(&pct, Some(150.0)).as_deref(), Some("≈ 101 M"));
        assert_eq!(reach_hint(&pct, None), None);
        assert_eq!(reach_hint(&Value::Number(45.0, Some(Unit::Metres)), Some(150.0)), None);
    }
}
