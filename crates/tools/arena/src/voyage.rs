//! Voyage fitness (track L2 of the evolution lab): a design (and a behavior) flown through the slice's voyages by
//! the headless voyage runner (`sim::voyage::runner`), scored 0..1 by the goal cleared, the time and the hull lost.

use data::Content;
use sim::{BehaviorLibrary, Flagship, SimConfig, VoyageRun, VoyageRunner};

use crate::error::ArenaError;
use crate::fight::{DesignProblem, resolve_design};
use crate::spec::{BehaviorRef, DesignRef};

/// Time limit of a voyage run when nothing else is said (minutes). The scripted runs take 2.5–3.5 minutes
/// (`docs/sim.md`); a run that is out of time scores as not cleared. PLACEHOLDER.
pub const DEFAULT_VOYAGE_MINUTES: f32 = 10.0;
/// Longest allowed voyage run (minutes), a sanity bound.
pub const MAX_VOYAGE_MINUTES: f32 = 600.0;

/// One voyage run: which voyage, the seed (the voyage's roll and the world), the design, the behavior it fights
/// with and the time limit.
#[derive(Clone, Debug, PartialEq)]
pub struct VoyageSpec {
    /// A content voyage id.
    pub voyage: String,
    pub seed: u64,
    pub design: DesignRef,
    /// Flies this behavior in fights; `None`: the runner's scripted closing pilot.
    pub behavior: Option<BehaviorRef>,
    pub minutes: f32,
}

impl VoyageSpec {
    /// The time limit in ticks (at least one).
    pub fn limit_ticks(&self) -> u64 {
        ((self.minutes * 60.0 * sim::TICK_HZ as f32).round() as u64).max(1)
    }
}

/// Builds the run: checks the spec, compiles the design and the behavior, rolls the voyage. Step the runner
/// (`VoyageRunner::step`) or run it to the end ([`run_voyage`]).
pub fn start_voyage(content: &Content, spec: &VoyageSpec) -> Result<VoyageRunner, ArenaError> {
    let m = spec.minutes;
    if !(m.is_finite() && m > 0.0 && m <= MAX_VOYAGE_MINUTES) {
        return Err(ArenaError::Number {
            field: "minutes",
            value: m,
            max: MAX_VOYAGE_MINUTES,
        });
    }
    let voyage = content
        .voyages
        .get(&spec.voyage)
        .ok_or_else(|| ArenaError::UnknownVoyage(spec.voyage.clone()))?;
    let (bp, design) = resolve_design(content, &spec.design).map_err(|p| match p {
        DesignProblem::UnknownShip(id) => ArenaError::Voyage(format!("no ship `{id}` in the content")),
        DesignProblem::Blueprint(problems) => ArenaError::Voyage(format!(
            "the blueprint is not usable: {}",
            problems.join("; ")
        )),
    })?;
    let report = sim::design_report(content, &bp, &SimConfig::default()).map_err(ArenaError::Voyage)?;
    let mut lib = BehaviorLibrary::new(content).map_err(|p| {
        ArenaError::Content(
            p.iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    let behavior = match &spec.behavior {
        None => None,
        Some(BehaviorRef::Content(id)) => Some(
            lib.id(id)
                .ok_or_else(|| ArenaError::Voyage(format!("no behavior `{id}` in the content")))?,
        ),
        Some(BehaviorRef::Text(text)) => Some(lib.add_text("arena_voyage", text).map_err(|errs| {
            ArenaError::Voyage(format!(
                "the behavior text is not usable: {}",
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        })?),
    };
    let flagship = Flagship {
        design: std::sync::Arc::new(design),
        reach: sim::closing_reach(&report),
        behavior,
    };
    VoyageRunner::new(
        content,
        voyage,
        flagship,
        lib,
        &[],
        spec.seed,
        spec.limit_ticks(),
    )
    .map_err(ArenaError::Voyage)
}

/// Builds the run and flies it to the end.
pub fn run_voyage(content: &Content, spec: &VoyageSpec) -> Result<VoyageRun, ArenaError> {
    Ok(start_voyage(content, spec)?.run())
}

/// A run's score, 0..1 (PLACEHOLDER weights, `docs/arena.md` "Voyage fitness"):
/// - the goal cleared and home: 0.6 + 0.2 × (1 − share of the hull lost) + 0.2 × (1 − ticks / `limit_ticks`);
/// - else 0.5 × (sites cleared / sites with ships), halved when the ship was lost.
///
/// So any cleared voyage beats any uncleared one, a faster and less damaged clear beats a slower one, and
/// clearing more sites counts before the goal falls.
pub fn voyage_score(run: &VoyageRun, limit_ticks: u64) -> f32 {
    if run.cleared_and_home() {
        let time = 1.0 - (run.ticks as f32 / limit_ticks.max(1) as f32).clamp(0.0, 1.0);
        0.6 + 0.2 * (1.0 - run.hull_lost_share()) + 0.2 * time
    } else {
        let progress = if run.sites_with_ships > 0 {
            run.sites_cleared as f32 / run.sites_with_ships as f32
        } else {
            0.0
        };
        let lost = if run.outcome == sim::voyage::runner::Outcome::Lost {
            0.5
        } else {
            1.0
        };
        (0.5 * progress * lost).clamp(0.0, 0.5)
    }
}

/// The voyage fitness of a design and behavior: the mean [`voyage_score`] over every voyage in `voyages` and
/// every seed in `seeds` (the spec's own voyage and seed are replaced), run in order.
pub fn voyage_fitness(
    content: &Content,
    spec: &VoyageSpec,
    voyages: &[String],
    seeds: &[u64],
) -> Result<f32, ArenaError> {
    let mut sum = 0.0f64;
    let mut n = 0u32;
    let mut s = spec.clone();
    for v in voyages {
        for &seed in seeds {
            s.voyage = v.clone();
            s.seed = seed;
            sum += voyage_score(&run_voyage(content, &s)?, s.limit_ticks()) as f64;
            n += 1;
        }
    }
    Ok(if n > 0 { (sum / n as f64) as f32 } else { 0.0 })
}
