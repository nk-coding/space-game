//! Combat and what it leaves: weapons (shots, beams, shields), engine plumes on hulls, chain reactions of
//! exploding parts, and salvage (wrecks, loose items, cargo); mining beams cutting ore from rock (`mining`).

pub mod chain;
pub mod mining;
pub mod missile;
pub(crate) mod plume;
pub mod salvage;
pub mod weapons;
