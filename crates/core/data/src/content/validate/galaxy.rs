//! Validation of the galaxy's kinds (part of [`Validator`]'s impl): galaxies, bands, cluster types, trails, signal
//! sources and lore fragments.

use super::*;

impl<'a> Validator<'a> {
    /// A range `(min, max)` of numbers above 0 with `min ≤ max ≤ cap`.
    fn span<T: Entry>(&mut self, e: &T, field: &str, (lo, hi): (f32, f32), cap: f32, unit: &str) {
        if !(lo.is_finite() && hi.is_finite() && lo > 0.0 && lo <= hi && hi <= cap) {
            self.err(e, field, format!("`(min, max)` with 0 < min ≤ max ≤ {cap}{unit}, got ({lo}, {hi})"));
        }
    }

    /// A list of ids of one kind, none twice.
    fn ids<T: Entry>(&mut self, e: &T, field: &str, ids: &[String], kind: Kind) {
        for (k, id) in ids.iter().enumerate() {
            if !self.c.exists(kind, id) {
                self.err(e, format!("{field}[{k}]"), format!("unknown {kind} `{id}`"));
            } else if ids[..k].contains(id) {
                self.err(e, format!("{field}[{k}]"), format!("`{id}` is listed twice"));
            }
        }
    }

    /// A trail (id) whose reward is `reward`.
    fn trail_ref<T: Entry>(&mut self, e: &T, field: &str, id: &str, reward: &[Reward]) {
        match self.c.trails.get(id) {
            None => self.err(e, field, format!("unknown trail `{id}`")),
            Some(t) if !reward.contains(&t.reward) => self.err(
                e,
                field,
                format!("trail `{id}` leads to {:?}; this needs one that leads to {}", t.reward, reward.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>().join(" or ")),
            ),
            Some(_) => {}
        }
    }

    /// A galaxy: its bands from the rim inwards with shrinking edges, gates on every band but the innermost, and
    /// numbers the generator can work with.
    pub(super) fn galaxy(&mut self, g: &'a Galaxy) {
        let c = self.c;
        if g.bands.is_empty() {
            self.err(g, "bands", "a galaxy needs at least one band");
        }
        self.ids(g, "bands", &g.bands, Kind::Band);
        // The known bands with their places in the list.
        let bands: Vec<(usize, &Band)> = g.bands.iter().enumerate().filter_map(|(k, b)| Some((k, c.bands.get(b)?))).collect();
        for pair in bands.windows(2) {
            let ((_, outer), (k, inner)) = (pair[0], pair[1]);
            if inner.edge >= outer.edge {
                self.err(g, format!("bands[{k}]"), format!("band `{}` must lie inside `{}`: its edge {} S is not below {} S", inner.id, outer.id, inner.edge, outer.edge));
            }
        }
        for (n, &(k, b)) in bands.iter().enumerate() {
            let last = n + 1 == bands.len();
            if last && b.gates.is_some() {
                self.err(g, format!("bands[{k}]"), format!("band `{}` is the innermost: it has no next band to open a gate to", b.id));
            } else if !last && b.gates.is_none() {
                self.err(g, format!("bands[{k}]"), format!("band `{}` has no gates, so the bands inside it can never be reached", b.id));
            }
        }
        if bands.first().is_some_and(|(_, b)| b.edge > MAX_GALAXY_RADIUS) {
            self.err(g, "bands[0]", format!("the galaxy is at most {MAX_GALAXY_RADIUS} S across its radius"));
        }
        self.number(g, "spacing", g.spacing, 0.0, true);
        self.number(g, "jump_range", g.jump_range, 0.0, true);
        // A cluster lies within `spacing` of every spot of its band, so neighbours are at most twice that apart.
        if g.jump_range < 2.0 * g.spacing {
            self.err(g, "jump_range", format!("at least twice the spacing ({} S), or a band's clusters may not all be reachable", 2.0 * g.spacing));
        }
        self.count(g, "home_neighbours", g.home_neighbours, MAX_HOME_NEIGHBOURS);
        self.number(g, "warp", g.warp.0, 0.0, false);
        self.number(g, "warp", g.warp.1, 0.0, false);
        self.span(g, "signal_area.across", g.signal_area.across, MAX_GALAXY_RADIUS, " S");
        let off = g.signal_area.offset;
        if !(off.is_finite() && (0.0..1.0).contains(&off)) {
            self.err(g, "signal_area.offset", format!("a share of the area's radius, 0 to below 1, got {off}"));
        }
        let narrow = g.signal_area.narrow;
        if !(narrow.is_finite() && narrow > 0.0 && narrow < 1.0) {
            self.err(g, "signal_area.narrow", format!("a share of the area's radius, above 0 and below 1, got {narrow}"));
        }
        self.span(g, "trail_step", g.trail_step, MAX_GALAXY_RADIUS, " S");
        for (field, parts) in [("names.starts", &g.names.starts), ("names.ends", &g.names.ends)] {
            if parts.is_empty() || parts.iter().any(|p| p.trim().is_empty()) {
                self.err(g, field, "at least one name part, none empty");
            }
        }
        self.set_pieces(g);
    }

    /// A galaxy's set pieces: voyages of its bands, each once; a gate's piece in a band with gates, one a band, with a
    /// goal that has ships to beat; the galaxy's voyages without a piece are never on the map (a warning).
    fn set_pieces(&mut self, g: &'a Galaxy) {
        let c = self.c;
        let mut gated: Vec<&str> = Vec::new();
        for (k, p) in g.set_pieces.iter().enumerate() {
            let f = format!("set_pieces[{k}]");
            self.span(g, &format!("{f}.distance"), p.distance, MAX_GALAXY_RADIUS, " S");
            let Some(v) = c.voyages.get(&p.voyage) else {
                self.err(g, format!("{f}.voyage"), format!("unknown voyage `{}`", p.voyage));
                continue;
            };
            if g.set_pieces[..k].iter().any(|q| q.voyage == p.voyage) {
                self.err(g, format!("{f}.voyage"), format!("`{}` is placed twice", p.voyage));
            }
            if !g.bands.contains(&v.band) {
                self.err(g, format!("{f}.voyage"), format!("voyage `{}` lies in band `{}`, not one of the galaxy's", v.id, v.band));
            }
            if !p.gate {
                continue;
            }
            if c.bands.get(&v.band).is_some_and(|b| b.gates.is_none()) {
                self.err(g, format!("{f}.gate"), format!("band `{}` has no gates", v.band));
            }
            if gated.contains(&v.band.as_str()) {
                self.err(g, format!("{f}.gate"), format!("band `{}` has a gate's piece already", v.band));
            }
            gated.push(&v.band);
            let guarded = v.goal().and_then(|s| c.encounters.get(&s.encounter)).is_some_and(|e| e.always_has_ships());
            if !guarded {
                self.err(g, format!("{f}.gate"), format!("voyage `{}` has no goal with ships for sure: its gate could never open", v.id));
            }
        }
        for v in c.voyages.iter().filter(|v| g.bands.contains(&v.band)) {
            if !g.set_pieces.iter().any(|p| p.voyage == v.id) {
                self.warn(g, "set_pieces", format!("voyage `{}` has no set piece: it is never on the map", v.id));
            }
        }
    }

    /// A band: an edge, cluster types with weights, gates that lead to a gate trail, trails to caches and salvage.
    pub(super) fn band(&mut self, b: &'a Band) {
        let c = self.c;
        self.number(b, "edge", b.edge, 0.0, true);
        if b.clusters.is_empty() {
            self.err(b, "clusters", "a band needs at least one cluster type");
        }
        for (k, w) in b.clusters.iter().enumerate() {
            if !c.cluster_types.contains(&w.id) {
                self.err(b, format!("clusters[{k}].id"), format!("unknown cluster_type `{}`", w.id));
            }
            if w.weight == 0 {
                self.err(b, format!("clusters[{k}].weight"), "a weight above 0");
            }
            self.count(b, &format!("clusters[{k}].weight"), w.weight, MAX_COUNT);
        }
        if let Some(gates) = &b.gates {
            if gates.count == 0 {
                self.err(b, "gates.count", "at least one gate (or no `gates` at all)");
            }
            self.count(b, "gates.count", gates.count, MAX_GATES);
            self.trail_ref(b, "gates.trail", &gates.trail, &[Reward::Gate]);
        }
        for (k, t) in b.trails.iter().enumerate() {
            self.trail_ref(b, &format!("trails[{k}].trail"), &t.trail, &[Reward::Cache, Reward::Salvage]);
            self.count(b, &format!("trails[{k}].count"), t.count, MAX_BAND_TRAILS);
        }
        if !c.galaxies.iter().any(|g| g.bands.contains(&b.id)) {
            self.warn(b, "id", "no galaxy lists this band");
        }
    }

    /// A cluster type: a radius within one `f32` space, site spacing that fits it, name words and known encounters.
    pub(super) fn cluster_type(&mut self, t: &'a ClusterType) {
        let c = self.c;
        self.span(t, "radius", t.radius, MAX_CLUSTER_RADIUS, " m");
        self.span(t, "spacing", t.spacing, MAX_VOYAGE_RADIUS, " m");
        if t.spacing.0 > 2.0 * t.radius.0 {
            self.err(t, "spacing", format!("sites at least {} m apart never fit into a radius of {} m", t.spacing.0, t.radius.0));
        }
        if t.words.is_empty() || t.words.iter().any(|w| w.trim().is_empty()) {
            self.err(t, "words", "at least one name word, none empty");
        }
        let mut most = 0u32;
        for (k, s) in t.sites.iter().enumerate() {
            if !c.encounters.contains(&s.encounter) {
                self.err(t, format!("sites[{k}].encounter"), format!("unknown encounter `{}`", s.encounter));
            }
            self.count_range(t, &format!("sites[{k}].count"), s.count, MAX_VOYAGE_SITES);
            most = most.saturating_add(s.count.1);
        }
        if most > MAX_VOYAGE_SITES {
            self.err(t, "sites", format!("at most {MAX_VOYAGE_SITES} sites in a cluster, up to {most} here"));
        }
        if !c.bands.iter().any(|b| b.clusters.iter().any(|w| w.id == t.id)) {
            self.warn(t, "id", "no band has clusters of this type");
        }
    }

    /// A trail: 1 to [`MAX_TRAIL_LENGTH`] signals, a target that can be cleared when it is a gate's guardian, known
    /// sources and a hint.
    pub(super) fn trail(&mut self, t: &'a Trail) {
        let c = self.c;
        let (lo, hi) = t.length;
        if lo == 0 || lo > hi || hi > MAX_TRAIL_LENGTH {
            self.err(t, "length", format!("`(min, max)` signals with 1 ≤ min ≤ max ≤ {MAX_TRAIL_LENGTH}, got ({lo}, {hi})"));
        }
        match c.encounters.get(&t.target) {
            None => self.err(t, "target", format!("unknown encounter `{}`", t.target)),
            Some(e) if t.reward == Reward::Gate && !e.always_has_ships() => {
                self.err(t, "target", format!("encounter `{}` has no ships for sure, so the gate's guardian could not be beaten", e.id))
            }
            Some(_) => {}
        }
        if t.sources.is_empty() {
            self.err(t, "sources", "a trail needs at least one signal source");
        }
        self.ids(t, "sources", &t.sources, Kind::SignalSource);
        if t.hint.trim().is_empty() {
            self.err(t, "hint", "hint must not be empty");
        }
        for (id, &x) in &t.gives.data {
            let f = format!("gives.data.{id}");
            if !c.families.contains(id) {
                self.err(t, &f, format!("unknown research family `{id}`"));
            }
            self.research_data(t, &f, x, true);
        }
        self.material_amounts(t, "gives.materials", &t.gives.materials);
        let used = match t.reward {
            Reward::Gate => c.bands.iter().any(|b| b.gates.as_ref().is_some_and(|g| g.trail == t.id)),
            Reward::Lore => c.lore.iter().any(|l| l.trail == t.id),
            Reward::Cache | Reward::Salvage => c.bands.iter().any(|b| b.trails.iter().any(|x| x.trail == t.id)),
        };
        if !used {
            self.warn(t, "id", "no band or lore fragment places this trail");
        }
    }

    /// A signal source: a log lies in an encounter with wrecks; a beacon's keeper, if any, is a known encounter.
    pub(super) fn signal_source(&mut self, s: &'a SignalSource) {
        let c = self.c;
        if s.line.trim().is_empty() {
            self.err(s, "line", "line must not be empty");
        }
        match (&s.site, s.form) {
            (None, SourceForm::Log) => self.err(s, "site", "a log lies in a wreck: name the encounter it lies in"),
            (None, SourceForm::Beacon) => {}
            (Some(id), form) => match c.encounters.get(id) {
                None => self.err(s, "site", format!("unknown encounter `{id}`")),
                Some(e) if form == SourceForm::Log && e.wrecks.is_empty() => {
                    self.err(s, "site", format!("encounter `{id}` has no wrecks for the log to lie in"))
                }
                Some(_) => {}
            },
        }
    }

    /// A lore fragment: a band of some galaxy, a trail that leads to lore, a text.
    pub(super) fn lore_fragment(&mut self, l: &'a LoreFragment) {
        if !self.c.bands.contains(&l.band) {
            self.err(l, "band", format!("unknown band `{}`", l.band));
        }
        self.trail_ref(l, "trail", &l.trail, &[Reward::Lore]);
        if l.text.trim().is_empty() {
            self.err(l, "text", "text must not be empty");
        }
    }
}
