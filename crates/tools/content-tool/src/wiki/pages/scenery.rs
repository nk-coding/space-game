//! The scenery's pages (GDR-0104): rock kinds (with a drawing of a few rocks rolled from the kind, the sim's own
//! outlines), landmarks, and the "Between the sites" section of cluster types and voyages.

use super::*;
use data::{Landmark, LandmarkForm, RockKind, RockLayout, Scenery};

/// How many rocks a rock kind's drawing shows.
const SAMPLES: u32 = 5;

/// A landmark's form in words.
pub(crate) fn form_word(f: LandmarkForm) -> &'static str {
    match f {
        LandmarkForm::Station => "a derelict station",
        LandmarkForm::Beacon => "a beacon",
        LandmarkForm::Ring => "a ring of junk",
        LandmarkForm::Corona => "a hot star's corona",
    }
}

/// A rock group's layout in words.
fn layout_word(l: RockLayout) -> String {
    match l {
        RockLayout::Field { radius } => format!("fields {} across", metres(2.0 * radius)),
        RockLayout::Belt { width } => format!("belts {} wide", metres(width)),
        RockLayout::Scatter => "scattered".into(),
    }
}

impl Wiki<'_> {
    /// A row of rocks rolled from the kind as the sim rolls them (`sim::voyage::rocks::outline`), side by side.
    fn rock_figure(&self, r: &RockKind) -> String {
        let fill = self.style.get(&format!("world.rock.{}", r.tone));
        let edge = self.style.get("world.rock_edge");
        let mut rng = sim::Pcg32::new(0x726f_636b, 1);
        let shapes: Vec<Vec<sim::Vec2>> = (0..SAMPLES).map(|_| sim::voyage::rocks::outline(r, &mut rng)).collect();
        let reach = |s: &Vec<sim::Vec2>| s.iter().fold(0.0f32, |m, p| m.max(p.length()));
        let width: f32 = shapes.iter().map(|s| 2.0 * reach(s) + 4.0).sum();
        let height = shapes.iter().map(|s| 2.0 * reach(s) + 4.0).fold(0.0f32, f32::max);
        let mut svg = format!(
            "<svg class=\"rocks\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"{} rocks\">",
            format_number(width, 1),
            format_number(height, 1),
            esc(&r.name)
        );
        let mut x = 0.0;
        for s in &shapes {
            let r0 = reach(s);
            let (cx, cy) = (x + r0 + 2.0, height / 2.0);
            let pts: Vec<String> = s.iter().map(|p| format!("{},{}", format_number(cx + p.x, 2), format_number(cy - p.y, 2))).collect();
            write!(svg, "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{edge}\" stroke-width=\"0.5\" stroke-linejoin=\"round\"/>", pts.join(" ")).unwrap();
            x += 2.0 * r0 + 4.0;
        }
        svg.push_str("</svg>");
        svg
    }

    /// Where a rock kind or landmark lies: every cluster type and voyage whose scenery names it, with `what` of it.
    fn scenery_users(&self, root: &str, what: impl Fn(&Scenery) -> Option<String>) -> String {
        let mut rows: Vec<String> = Vec::new();
        for t in &self.c.cluster_types {
            if let Some(w) = what(&t.scenery) {
                rows.push(format!("{}: {w}", self.a(root, Kind::ClusterType, &t.id)));
            }
        }
        for v in &self.c.voyages {
            if let Some(w) = what(&v.scenery) {
                rows.push(format!("{}: {w}", self.a(root, Kind::Voyage, &v.id)));
            }
        }
        list_or_none(&rows)
    }

    pub(crate) fn rock_page(&self, r: &RockKind) -> String {
        let root = UP;
        let facts = vec![
            ("Material".to_string(), self.a(root, Kind::Material, &r.material)),
            ("Radius".into(), format!("{}–{}", format_number(r.size.0, 1), metres(r.size.1))),
            ("Corners".into(), count_range(r.corners)),
            ("Roughness".into(), format!("up to {} % in from its radius", format_number(100.0 * r.roughness, 0))),
            ("Stretch".into(), format!("{}–{} ×", format_number(r.stretch.0, 2), format_number(r.stretch.1, 2))),
            (
                "Tone".into(),
                format!(
                    "<span class=\"dot tone\" style=\"--c: {}\"></span><code>{}</code>",
                    esc(&self.style.get(&format!("world.rock.{}", r.tone))),
                    esc(&r.tone)
                ),
            ),
        ];
        let what = "<p>Asteroid rock is anchored in place: it blocks ships, shots and engine plumes, so it gives cover. A ship that hits it takes ramming damage as in any ram, and behaviors steer round the rocks ahead. Each rock is rolled from its kind when the fleet warps in. Mining it comes later.</p>\n".to_string();
        let found = self.scenery_users(root, |s| {
            let groups: Vec<String> = s
                .rocks
                .iter()
                .filter(|g| g.rock == r.id)
                .map(|g| format!("{} ×{}, {} rocks each", layout_word(g.layout), count_range(g.count), count_range(g.rocks)))
                .collect();
            (!groups.is_empty()).then(|| groups.join("; "))
        });
        self.entry(
            r,
            EntryParts {
                figure: Some(self.rock_figure(r)),
                facts,
                sections: vec![("What it does".to_string(), what), ("Found in".into(), found)],
                ..Default::default()
            },
        )
    }

    pub(crate) fn landmark_page(&self, l: &Landmark) -> String {
        let root = UP;
        let facts = vec![
            ("Form".to_string(), form_word(l.form).into()),
            ("Size".into(), format!("{}–{}", format_number(l.size.0, 0), metres(l.size.1))),
            ("Depth".into(), format!("moves {} % as fast as the ships' plane", format_number(100.0 * l.parallax, 0))),
        ];
        let what = "<p>Seen far off behind the play, drawn only: it is no body, nothing can reach it, and it changes nothing in a fight.</p>\n".to_string();
        let seen = self.scenery_users(root, |s| {
            s.landmarks
                .iter()
                .find(|p| p.landmark == l.id)
                .map(|p| format!("{} % of clusters", format_number(100.0 * p.chance, 0)))
        });
        self.entry(
            l,
            EntryParts {
                facts,
                sections: vec![("What it is".to_string(), what), ("Seen in".into(), seen)],
                ..Default::default()
            },
        )
    }

    /// The "Between the sites" section of a cluster type's or a voyage's page; `None` for an empty scenery.
    pub(crate) fn scenery_section(&self, root: &str, s: &Scenery) -> Option<(String, String)> {
        if s.is_empty() {
            return None;
        }
        let mut out = String::new();
        if !s.rocks.is_empty() {
            out.push_str("<table class=\"list\">\n<tr><th>Rock</th><th>Lies in</th><th class=\"num\">How many</th><th class=\"num\">Rocks each</th></tr>\n");
            for g in &s.rocks {
                writeln!(
                    out,
                    "<tr><td class=\"name\">{}</td><td>{}</td><td class=\"num\">×{}</td><td class=\"num\">{}</td></tr>",
                    self.a(root, Kind::Rock, &g.rock),
                    layout_word(g.layout),
                    count_range(g.count),
                    count_range(g.rocks)
                )
                .unwrap();
            }
            out.push_str("</table>\n");
        }
        let mut facts: Vec<String> = Vec::new();
        let density = |x: f32| format!("{} × the base", format_number(x, 1));
        if s.dust > 0.0 {
            facts.push(format!("Dust and glints: {}", density(s.dust)));
        }
        if s.debris > 0.0 {
            facts.push(format!("Hull debris: {}, thicker near the sites", density(s.debris)));
        }
        if s.derelicts.1 > 0 {
            facts.push(format!("Derelict shells drifting by: {}", count_range(s.derelicts)));
        }
        for p in &s.landmarks {
            facts.push(format!("{} in {} % of clusters", self.a(root, Kind::Landmark, &p.landmark), format_number(100.0 * p.chance, 0)));
        }
        if s.fights > 0.0 {
            facts.push(format!("A fight flashing in the distance about {} times a minute", format_number(s.fights, 1)));
        }
        if !facts.is_empty() {
            out.push_str("<p class=\"note\">Drawn only, never in the way:</p>\n");
            out.push_str(&ul(&facts));
        }
        Some(("Between the sites".into(), out))
    }
}
