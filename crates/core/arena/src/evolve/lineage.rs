//! The genome table (GDR-0092 "What a run stores"): every genome a run breeds, numbered in breeding order, kept as a
//! change against its parent (the parts it lost and gained, in frame coordinates, and its behavior text and pilot
//! assists when they changed), so a run stores a few hundred bytes per child and still rebuilds any genome exactly, by replaying the
//! changes from its start.

use data::{AssistList, Blueprint, Placement};
use serde::{Deserialize, Serialize};

use super::genome::Genome;

/// One genome of a run's table: where it came from and how it differs from its parent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bred {
    /// The generation it was bred for.
    pub generation: u32,
    /// The start it descends from (its index in the run's starts).
    pub root: u32,
    /// The genome it was bred from; `None` for a start.
    pub parent: Option<u32>,
    /// The other parent of a crossover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner: Option<u32>,
    /// Indices of the parent's parts it lost, ascending.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<u32>,
    /// The parts it gained, each with its index among its own parts, ascending; in frame coordinates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<(u32, Placement)>,
    /// Its behavior text, when it differs from the parent's (always for a start).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior: Option<String>,
    /// Its pilot assists, when they differ from the parent's (no rules: none); a start's are its header's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assists: Option<AssistList>,
    /// The size of its blueprint (trimmed to its parts).
    pub size: (u32, u32),
}

/// Most cells of the table [`diff`] compares in full; past that a crossover's middle is stored whole (still exact).
const DIFF_CELLS: usize = 1 << 20;

/// The change from `parent` to `child` (both in frame coordinates): the parent's parts that go (ascending) and the
/// child's parts that are new (with their index in `child`). The parts both keep, in the same order, are the
/// longest common run of the two lists, so [`apply`] gives `child` back exactly.
pub(crate) fn diff(parent: &[Placement], child: &[Placement]) -> (Vec<u32>, Vec<(u32, Placement)>) {
    // A mutation changes a part or two: strip what both lists start and end with first.
    let pre = parent.iter().zip(child).take_while(|(a, b)| a == b).count();
    let suf = parent[pre..]
        .iter()
        .rev()
        .zip(child[pre..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (a, b) = (&parent[pre..parent.len() - suf], &child[pre..child.len() - suf]);
    let (n, m) = (a.len(), b.len());
    let index = |k: usize| (pre + k) as u32;
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    if (n + 1) * (m + 1) > DIFF_CELLS {
        removed.extend((0..n).map(index));
        added.extend(b.iter().enumerate().map(|(k, q)| (index(k), q.clone())));
        return (removed, added);
    }
    // keep[at(i, j)]: the length of the longest common subsequence of a[i..] and b[j..].
    let at = |i: usize, j: usize| i * (m + 1) + j;
    let mut keep = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            keep[at(i, j)] = if a[i] == b[j] {
                keep[at(i + 1, j + 1)] + 1
            } else {
                keep[at(i + 1, j)].max(keep[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if keep[at(i + 1, j)] >= keep[at(i, j + 1)] {
            removed.push(index(i));
            i += 1;
        } else {
            added.push((index(j), b[j].clone()));
            j += 1;
        }
    }
    removed.extend((i..n).map(index));
    added.extend((j..m).map(|k| (index(k), b[k].clone())));
    (removed, added)
}

/// `parent`'s parts with a change applied ([`diff`]).
pub(crate) fn apply(parent: &[Placement], removed: &[u32], added: &[(u32, Placement)]) -> Vec<Placement> {
    let mut parts: Vec<Placement> = parent
        .iter()
        .enumerate()
        .filter(|(k, _)| removed.binary_search(&(*k as u32)).is_err())
        .map(|(_, p)| p.clone())
        .collect();
    for (k, p) in added {
        parts.insert((*k as usize).min(parts.len()), p.clone());
    }
    parts
}

/// Genome `id` of `table`, rebuilt from its start through every change on the way, with its start's header
/// (`headers[root]`). `None` if the table does not lead back to a start (a damaged run file).
pub(crate) fn rebuild(table: &[Bred], headers: &[Blueprint], id: u32) -> Option<Genome> {
    let mut chain = Vec::new();
    let mut k = Some(id);
    while let Some(n) = k {
        let bred = table.get(n as usize)?;
        // Parents are bred before their children, so the chain ends.
        if bred.parent.is_some_and(|p| p >= n) || chain.len() > table.len() {
            return None;
        }
        chain.push(bred);
        k = bred.parent;
    }
    let last = table.get(id as usize)?;
    let header = headers.get(last.root as usize)?;
    let mut parts = Vec::new();
    let mut behavior = None;
    let mut assists = header.assists.clone();
    for b in chain.iter().rev() {
        parts = apply(&parts, &b.removed, &b.added);
        if let Some(t) = &b.behavior {
            behavior = Some(t);
        }
        if let Some(a) = &b.assists {
            assists = (!a.rules.is_empty()).then(|| a.clone());
        }
    }
    let at = (
        parts.iter().map(|p| p.x).min().unwrap_or(0),
        parts.iter().map(|p| p.y).min().unwrap_or(0),
    );
    Some(Genome {
        blueprint: Blueprint {
            size: last.size,
            parts: parts
                .into_iter()
                .map(|p| Placement {
                    x: p.x - at.0,
                    y: p.y - at.1,
                    ..p
                })
                .collect(),
            assists,
            ..header.clone()
        },
        behavior: behavior?.clone(),
        root: last.root,
        at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(part: &str, x: i32) -> Placement {
        Placement {
            part: part.into(),
            x,
            y: 0,
            rot: 0,
            flip: false,
            material: None,
            power: None,
            group: None,
        }
    }

    #[test]
    fn a_change_gives_the_child_back() {
        let a: Vec<Placement> = (0..12).map(|k| p("a", k)).collect();
        let cases: Vec<Vec<Placement>> = vec![
            a.clone(),
            a[1..].to_vec(),
            [a.clone(), vec![p("b", 30)]].concat(),
            // A part changed in place, one removed in the middle, a crossover's scattered removals and tail.
            [a[..5].to_vec(), vec![p("b", 5)], a[6..].to_vec()].concat(),
            [a[..3].to_vec(), a[4..].to_vec()].concat(),
            a.iter().filter(|q| q.x % 3 != 0).cloned().chain([p("c", 1), p("c", 2)]).collect(),
            Vec::new(),
        ];
        for child in cases {
            let (removed, added) = diff(&a, &child);
            assert!(removed.windows(2).all(|w| w[0] < w[1]));
            assert_eq!(apply(&a, &removed, &added), child);
            // What both keep is not stored again.
            assert_eq!(a.len() - removed.len(), child.len() - added.len());
        }
        let (removed, added) = diff(&a, &[a.clone(), vec![p("b", 30)]].concat());
        assert!(removed.is_empty() && added.len() == 1);
        let moved = [a[..5].to_vec(), vec![p("b", 5)], a[6..].to_vec()].concat();
        assert_eq!(diff(&a, &moved), (vec![5], vec![(5, p("b", 5))]));
    }
}
