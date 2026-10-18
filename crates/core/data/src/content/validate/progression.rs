//! Validation of progression (part of [`Validator`]'s impl): research nodes and their cycles, starts.

use super::*;

impl<'a> Validator<'a> {
    pub(super) fn research(&mut self, n: &'a ResearchNode) {
        let c = self.c;
        self.family_ref(n, "family", &n.family);
        for (id, &x) in &n.cost.data {
            let f = format!("cost.data.{id}");
            if !c.families.contains(id) {
                self.err(n, &f, format!("unknown research family `{id}`"));
            }
            self.research_data(n, &f, x, true);
        }
        self.material_amounts(n, "cost.materials", &n.cost.materials);
        for (k, r) in n.requires.iter().enumerate() {
            let f = format!("requires[{k}]");
            if !c.research.contains(r) {
                self.err(n, &f, format!("unknown research node `{r}`"));
            } else if n.requires[..k].contains(r) {
                self.err(n, &f, format!("`{r}` is listed twice"));
            }
        }
        if n.unlocks.is_empty() {
            self.warn(n, "unlocks", "the node unlocks nothing");
        }
        for (k, u) in n.unlocks.iter().enumerate() {
            let f = format!("unlocks[{k}]");
            let (kind, id, locked) = match u {
                Unlock::MindCapacity(x) => {
                    self.mind_capacity(n, &f, *x);
                    continue;
                }
                Unlock::JumpRange(x) => {
                    self.number(n, &f, *x, 0.0, true);
                    if *x > MAX_GALAXY_RADIUS {
                        self.err(n, &f, format!("a galaxy-map distance, at most {MAX_GALAXY_RADIUS} S"));
                    }
                    continue;
                }
                Unlock::PartStat { target, stat, step } => {
                    self.stat_change(n, &f, ("research steps", "step"), target, stat, *step);
                    let lands = c.parts.iter().filter(|p| p.matches(target)).any(|p| {
                        c.part_stats(p, None).iter().any(|l| l.bonus.is_none() && crate::design::upgrades::shows_on(stat, &l.stat))
                    });
                    if !lands && c.stats.contains(stat) {
                        self.err(n, &f, format!("no part it picks has `{stat}`: the step would change nothing"));
                    }
                    continue;
                }
                Unlock::Part(id) => (Kind::Part, id, c.parts.get(id).map(|p| p.locked)),
                Unlock::Material(id) => (Kind::Material, id, c.materials.get(id).map(|m| m.locked)),
                Unlock::Frame(id) => (Kind::Frame, id, c.frames.get(id).map(|x| x.locked)),
                Unlock::Facility(id) => (Kind::Facility, id, c.facilities.get(id).map(|x| x.locked)),
            };
            match locked {
                None => self.err(n, &f, format!("unknown {kind} `{id}`")),
                Some(false) => self.warn(
                    n,
                    &f,
                    format!(
                        "{kind} `{id}` is not `locked`: a new game has it already, so unlocking it changes nothing"
                    ),
                ),
                Some(true) => {}
            }
        }
    }

    /// A list of requirements (`requires` of research nodes, `opens_after` of voyages) must not loop: each cycle
    /// is reported once, at the requirement that closes it. Messages read "each <verb> the next" and "so none of
    /// them <outcome>".
    pub(super) fn cycles<T: Entry>(
        &mut self,
        table: &'a Table<T>,
        edges: impl Fn(&T) -> &[String],
        field: &str,
        verb: &str,
        outcome: &str,
    ) {
        let nodes = table.as_slice();
        // 0: not visited, 1: on the current path, 2: done.
        let mut state = vec![0u8; nodes.len()];
        for root in 0..nodes.len() {
            if state[root] != 0 {
                continue;
            }
            state[root] = 1;
            // Depth first without recursion (mods may chain many entries): (entry, next requirement to follow).
            let mut path: Vec<(usize, usize)> = vec![(root, 0)];
            while let Some(top) = path.last_mut() {
                let (node, k) = *top;
                let Some(r) = edges(&nodes[node]).get(k) else {
                    state[node] = 2;
                    path.pop();
                    continue;
                };
                top.1 += 1;
                let Some(j) = table.position(r) else {
                    continue; // an unknown id: reported by the entry's own check
                };
                match state[j] {
                    0 => {
                        state[j] = 1;
                        path.push((j, 0));
                    }
                    1 => {
                        let from = path.iter().position(|&(x, _)| x == j).unwrap();
                        let cycle: Vec<&str> = path[from..]
                            .iter()
                            .map(|&(x, _)| nodes[x].id())
                            .chain([nodes[j].id()])
                            .collect();
                        self.err(
                            &nodes[node],
                            format!("{field}[{k}]"),
                            format!(
                                "`{field}` forms a cycle ({}, each {verb} the next), so none of them {outcome}",
                                cycle.join(" → ")
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    /// A start's ship must be one a new game can take on a voyage: built from what is not locked, fitting a frame
    /// that is not locked, with a logic core and no more logic cores than the start's mind capacity; without a hold
    /// (a warning) nothing salvaged comes home, so the career can never research anything.
    /// A facility: an upgrade sits `within` a facility that is no upgrade itself; workshop rules have shares in
    /// 0..=1.
    pub(super) fn facility(&mut self, f: &'a Facility) {
        if let Some(w) = &f.within {
            match self.c.facilities.get(w) {
                None => self.err(f, "within", format!("unknown facility `{w}`")),
                Some(x) if x.within.is_some() => self.err(f, "within", format!("`{w}` is an upgrade itself")),
                Some(_) => {}
            }
        }
        if let Some(r) = &f.workshop {
            for (field, x) in [("workshop.dismantle", r.dismantle), ("workshop.broken", r.broken)] {
                if !(x.is_finite() && (0.0..=1.0).contains(&x)) {
                    self.err(f, field, format!("a share of 0 to 1, got {x}"));
                }
            }
        }
    }

    pub(super) fn start(&mut self, s: &'a Start) {
        let c = self.c;
        self.mind_capacity(s, "mind_capacity", s.mind_capacity);
        self.material_amounts(s, "materials", &s.materials);
        for (k, v) in s.voyages.iter().enumerate() {
            let f = format!("voyages[{k}]");
            if !c.voyages.contains(v) {
                self.err(s, &f, format!("unknown voyage `{v}`"));
            } else if s.voyages[..k].contains(v) {
                self.err(s, &f, format!("`{v}` is listed twice"));
            }
        }
        let Some(ship) = c.ships.get(&s.ship) else {
            self.err(s, "ship", format!("unknown ship `{}`", s.ship));
            return;
        };
        let mut locked = BTreeSet::new();
        for pl in &ship.parts {
            if c.parts.get(&pl.part).is_some_and(|p| p.locked) {
                locked.insert(format!("part `{}`", pl.part));
            }
            if let Some(m) = c.placement_material(pl).filter(|m| m.locked) {
                locked.insert(format!("material `{}`", m.id));
            }
        }
        if !locked.is_empty() {
            let list: Vec<String> = locked.into_iter().collect();
            self.err(
                s,
                "ship",
                format!(
                    "ship `{}` uses what a new game doesn't have: {}",
                    s.ship,
                    list.join(", ")
                ),
            );
        }
        let (w, h) = ship.size;
        if !c
            .frames
            .iter()
            .any(|f| !f.locked && f.size.0 >= w && f.size.1 >= h)
        {
            self.err(
                s,
                "ship",
                format!(
                    "ship `{}` ({w}×{h}) fits no frame that is not locked",
                    s.ship
                ),
            );
        }
        let cores = c.ship_totals(ship).logic_cores;
        if cores == 0 {
            self.err(
                s,
                "ship",
                format!("ship `{}` has no logic core: nothing could fly it", s.ship),
            );
        } else if c.ship_hold(ship) == 0 {
            self.warn(
                s,
                "ship",
                format!(
                    "ship `{}` has no hold (cargo on its main piece): nothing salvaged would come home, so \
                     research could never start",
                    s.ship
                ),
            );
        }
        if cores > s.mind_capacity {
            self.err(
                s,
                "mind_capacity",
                format!(
                    "ship `{}` has {cores} logic cores, more than the mind capacity {}",
                    s.ship, s.mind_capacity
                ),
            );
        }
    }
}
