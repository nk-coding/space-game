//! Validation of encounters and voyages (part of [`Validator`]'s impl), and that the voyages open one another.

use super::*;

impl<'a> Validator<'a> {
    /// A count range `(min, max)`: `min ≤ max ≤ max_count`.
    pub(super) fn count_range<T: Entry>(&mut self, e: &T, field: &str, (lo, hi): (u32, u32), max_count: u32) {
        if lo > hi {
            self.err(e, field, format!("min {lo} is above max {hi}"));
        } else if hi > max_count {
            self.err(e, field, format!("at most {max_count}, got {hi}"));
        }
    }

    /// A distance in metres: above 0 and at most [`MAX_VOYAGE_RADIUS`].
    pub(super) fn distance<T: Entry>(&mut self, e: &T, field: &str, x: f32) {
        if !(x.is_finite() && x > 0.0 && x <= MAX_VOYAGE_RADIUS) {
            self.err(
                e,
                field,
                format!("a distance above 0 and at most {MAX_VOYAGE_RADIUS} m, got {x}"),
            );
        }
    }

    /// An encounter's ships must be able to fly (a logic core) and should fight (a behavior); its wrecks may be
    /// any ship, with a share of their parts gone below 1.
    pub(super) fn encounter(&mut self, e: &'a Encounter) {
        let c = self.c;
        self.distance(e, "wake_radius", e.wake_radius);
        if e.ships.is_empty() && e.wrecks.is_empty() {
            self.err(e, "ships", "an encounter needs ships or wrecks");
        }
        for (k, s) in e.ships.iter().enumerate() {
            let f = format!("ships[{k}]");
            self.count_range(e, &format!("{f}.count"), s.count, MAX_ENCOUNTER_COUNT);
            let Some(ship) = c.ships.get(&s.ship) else {
                self.err(e, format!("{f}.ship"), format!("unknown ship `{}`", s.ship));
                continue;
            };
            if c.ship_totals(ship).logic_cores == 0 {
                self.err(
                    e,
                    format!("{f}.ship"),
                    format!(
                        "ship `{}` has no logic core, so it would spawn as salvage; list it under `wrecks`",
                        s.ship
                    ),
                );
            } else if c.ship_behavior(ship).is_none() {
                self.warn(
                    e,
                    format!("{f}.ship"),
                    format!(
                        "ship `{}` has no behavior (its own or its faction's): it would hold and never fight",
                        s.ship
                    ),
                );
            }
        }
        for (k, w) in e.wrecks.iter().enumerate() {
            let f = format!("wrecks[{k}]");
            self.count_range(e, &format!("{f}.count"), w.count, MAX_ENCOUNTER_COUNT);
            if !c.ships.contains(&w.ship) {
                self.err(e, format!("{f}.ship"), format!("unknown ship `{}`", w.ship));
            }
            if !(w.damage.is_finite() && (0.0..1.0).contains(&w.damage)) {
                self.err(
                    e,
                    format!("{f}.damage"),
                    format!("a share of the parts, 0 to below 1, got {}", w.damage),
                );
            }
        }
    }

    /// A voyage: its numbers in range, its sites' encounters known, exactly one goal that can be cleared, and
    /// `opens_after` naming other voyages (loops: [`Validator::cycles`]).
    pub(super) fn voyage(&mut self, v: &'a Voyage) {
        let c = self.c;
        if !is_snake_id(&v.band) {
            self.err(
                v,
                "band",
                format!("band key `{}` must be snake_case", v.band),
            );
        }
        for (k, id) in v.opens_after.iter().enumerate() {
            let f = format!("opens_after[{k}]");
            if !c.voyages.contains(id) {
                self.err(v, &f, format!("unknown voyage `{id}`"));
            } else if v.opens_after[..k].contains(id) {
                self.err(v, &f, format!("`{id}` is listed twice"));
            }
        }
        if v.opens_after.is_empty() && !c.starts.iter().any(|s| s.voyages.contains(&v.id)) {
            self.warn(
                v,
                "opens_after",
                "no start opens it and it opens after no other voyage, so it can never be launched",
            );
        }
        self.distance(v, "radius", v.radius);
        self.scenery(v, &v.scenery, v.radius, false);
        let (near, far) = v.spacing;
        self.distance(v, "spacing", near);
        self.distance(v, "spacing", far);
        if near > far {
            self.err(v, "spacing", format!("min {near} is above max {far}"));
        } else if near > 2.0 * v.radius {
            self.err(
                v,
                "spacing",
                format!(
                    "sites at least {near} m apart never fit into a radius of {} m",
                    v.radius
                ),
            );
        }
        if v.minutes.0 == 0 {
            self.err(v, "minutes", "at least 1 minute");
        }
        self.count_range(v, "minutes", v.minutes, MAX_VOYAGE_MINUTES);
        self.gives(v, &v.gives);
        if v.sites.is_empty() {
            self.err(v, "sites", "a voyage needs at least one site");
            return;
        }
        let mut goals = 0;
        for (k, s) in v.sites.iter().enumerate() {
            let f = format!("sites[{k}]");
            if s.count == 0 {
                self.err(v, format!("{f}.count"), "at least 1");
            }
            let encounter = c.encounters.get(&s.encounter);
            if encounter.is_none() {
                self.err(
                    v,
                    format!("{f}.encounter"),
                    format!("unknown encounter `{}`", s.encounter),
                );
            }
            if !s.goal {
                continue;
            }
            goals += 1;
            if goals > 1 {
                self.err(v, format!("{f}.goal"), "only one site can be the goal");
            }
            if s.count != 1 {
                self.err(
                    v,
                    format!("{f}.count"),
                    format!("the goal is one site, got a count of {}", s.count),
                );
            }
            if let Some(e) = encounter {
                if !e.always_has_ships() {
                    self.err(
                        v,
                        format!("{f}.encounter"),
                        format!(
                            "encounter `{}` may roll no ships, so the goal could never be cleared",
                            e.id
                        ),
                    );
                } else if !e.ships.iter().any(|s| {
                    s.count.0 > 0 && c.ships.get(&s.ship).is_some_and(|b| c.ship_armed(b))
                }) {
                    self.err(
                        v,
                        format!("{f}.encounter"),
                        format!(
                            "encounter `{}` may roll no armed ship (power and a weapon, or a ram prow and an engine, on its main piece): a site \
                             clears once its enemies are destroyed or disarmed (GDR-0068), so the goal would clear \
                             the moment it wakes",
                            e.id
                        ),
                    );
                }
            }
        }
        if goals == 0 {
            self.err(v, "sites", "no site is the goal: mark one `goal: true`");
        }
        let total = v.site_count();
        if total > MAX_VOYAGE_SITES {
            self.err(
                v,
                "sites",
                format!("at most {MAX_VOYAGE_SITES} sites in all, got {total}"),
            );
        }
    }

    /// Every voyage must be able to open (`docs/content-model.md` "Voyages"): through a start, or after voyages
    /// that can all open. A voyage with `opens_after` that only unreachable voyages lead to is a warning (one with
    /// none and no start is warned about in [`Validator::voyage`]; loops in [`Validator::cycles`]).
    pub(super) fn voyages_open(&mut self) {
        let c = self.c;
        let voyages = c.voyages.as_slice();
        let mut open: Vec<bool> = voyages
            .iter()
            .map(|v| c.starts.iter().any(|s| s.voyages.contains(&v.id)))
            .collect();
        loop {
            let mut changed = false;
            for (k, v) in voyages.iter().enumerate() {
                if !open[k]
                    && !v.opens_after.is_empty()
                    && v.opens_after
                        .iter()
                        .all(|id| c.voyages.position(id).is_some_and(|j| open[j]))
                {
                    open[k] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (k, v) in voyages.iter().enumerate() {
            if open[k] || v.opens_after.is_empty() {
                continue;
            }
            let stuck: Vec<&str> = v
                .opens_after
                .iter()
                .filter(|id| c.voyages.position(id).is_none_or(|j| !open[j]))
                .map(String::as_str)
                .collect();
            self.warn(
                v,
                "opens_after",
                format!(
                    "it opens after `{}`, which can never open, so it can never be launched",
                    stuck.join("`, `")
                ),
            );
        }
    }
}
