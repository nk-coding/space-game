//! The rules' fingerprint (GDR-0092): which rules a stored fight was fought under, so a replay can say at once
//! whether they have changed since. FNV-1a 64 over the content's fingerprint ([`Content::fingerprint`]) and the
//! final state hashes of the rules probes: short fixed fights (`probes/*.ron`, 300 ticks each, a few ms natively)
//! whose end changes with almost any change to the sim's code, without a version number someone must remember to
//! raise. What the probes miss, the hash check at the end of every replay catches.

use data::Content;

use crate::fight::run_fight;
use crate::spec::FightSpec;

/// The rules probes, compiled in (the web build has no files).
const PROBES: [&str; 2] = [
    include_str!("../probes/close_quarters.ron"),
    include_str!("../probes/warden.ron"),
];

/// The fingerprint of the rules `content` and this build's sim make. Equal on every platform for equal rules.
pub fn rules_fingerprint(content: &Content) -> u64 {
    let mut bytes = content.fingerprint().to_le_bytes().to_vec();
    for text in PROBES {
        // A probe that doesn't build on this content (a ship it lost) counts as 0; the content's fingerprint
        // differs then anyway.
        let end = FightSpec::from_ron(text)
            .ok()
            .and_then(|spec| run_fight(content, &spec).ok())
            .map_or(0, |r| r.state_hash);
        bytes.extend(end.to_le_bytes());
    }
    data::hash::fnv1a64(&bytes)
}
