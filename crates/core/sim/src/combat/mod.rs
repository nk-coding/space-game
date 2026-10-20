//! Combat and what it leaves: weapons (shots, beams, shields), engine plumes on hulls, chain reactions of
//! exploding parts, and salvage (wrecks, loose items, cargo); flares luring missiles (`flare`); mining beams cutting ore from rock (`mining`); static from disruption guns blinding minds (`disruption`) and EMP bursts (`emp`); lances striking through a hull (`lance`); tractor beams pulling salvage in (`tractor`).

pub mod chain;
pub mod disruption;
pub mod emp;
pub mod flare;
pub mod lance;
pub mod mining;
pub mod missile;
pub(crate) mod plume;
pub mod salvage;
pub mod tractor;
pub mod weapons;
