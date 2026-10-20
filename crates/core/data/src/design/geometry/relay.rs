//! The prism relay's beam path (GDR-0126, the Lattice). A gun fires through its facing side. When the first part in
//! its line of fire is a prism that takes its beam (the prism's `target` picks the gun) and the beam enters any of
//! the prism's sides but its front, the prism sends the beam on from its front side, and so on through a chain of
//! prisms: the beam may turn at a prism. The chain ends where the beam meets no part (the line of fire is clear,
//! the shot leaves from the last prism's front) or a part that does not take it (blocked). At most [`MAX_PRISMS`]
//! prisms; a prism met twice blocks. Only parts that `counts` keeps stand in the way (the sim: alive parts of the
//! gun's piece; the drydock: every part), so where a prism was shot out the beam crosses the gap to the next part
//! in line. A gun wider than one cell relays only when every cell of its front meets the same prism first.

use super::{Cell, Dir, ShipGrid};

/// Most prisms in one chain (GDR-0126; PLACEHOLDER, lead default 20 Oct 2026).
pub const MAX_PRISMS: usize = 4;

/// Where a gun's beam goes through its ship ([`ShipGrid::beam_path`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamPath {
    /// The prisms the beam passes, in order (`prisms[..len]`).
    pub prisms: [u32; MAX_PRISMS],
    pub len: u8,
    /// The part the beam leaves through (the gun, or the last prism) and the side it leaves by.
    pub exit: usize,
    pub dir: Dir,
    /// The first part in the way after the exit: the line of fire is blocked. `None`: clear.
    pub blocker: Option<usize>,
}

impl BeamPath {
    /// The prisms the beam passes, in order.
    pub fn prisms(&self) -> &[u32] {
        &self.prisms[..self.len as usize]
    }

    pub fn clear(&self) -> bool {
        self.blocker.is_none()
    }
}

impl ShipGrid {
    /// The beam path of gun `gun` (see the module). `counts(o)`: part `o` stands in a beam's way; `prism(o)`: the
    /// facing of part `o` if it is a prism that takes this gun's beam.
    pub fn beam_path(
        &self,
        gun: usize,
        counts: impl Fn(usize) -> bool,
        prism: impl Fn(usize) -> Option<Dir>,
    ) -> BeamPath {
        let mut path = BeamPath {
            prisms: [0; MAX_PRISMS],
            len: 0,
            exit: gun,
            dir: self.shape(gun).facing(),
            blocker: None,
        };
        loop {
            let (e, d) = (path.exit, path.dir);
            let (dx, dy) = d.delta();
            let mut first: Option<usize> = None;
            let (mut same, mut open) = (true, false);
            for c in self.border_cells(e, d) {
                let mut p: Cell = (c.0 + dx, c.1 + dy);
                let mut hit = None;
                while self.slot(p).is_some() {
                    if let Some(o) = self.occupant(p)
                        && o != e
                        && counts(o)
                    {
                        hit = Some(o);
                        break;
                    }
                    p = (p.0 + dx, p.1 + dy);
                }
                match (hit, first) {
                    (None, _) => open = true,
                    (Some(o), None) => first = Some(o),
                    (Some(o), Some(f)) => same &= o == f,
                }
            }
            let Some(o) = first else { return path };
            let relays = !open
                && same
                && (path.len as usize) < MAX_PRISMS
                && !path.prisms().contains(&(o as u32))
                && prism(o).is_some_and(|f| f != d.opposite());
            if !relays {
                path.blocker = Some(o);
                return path;
            }
            path.prisms[path.len as usize] = o as u32;
            path.len += 1;
            path.exit = o;
            path.dir = self.shape(o).facing();
        }
    }

    /// The centre of the side `dir` of part `i` (blueprint frame): where a gun's shots leave (its muzzle), or a
    /// relayed beam's from the last prism.
    pub fn side_mid(&self, i: usize, dir: Dir) -> [f32; 2] {
        let s = self.shape(i);
        let (w, h) = s.dims();
        let (x, y, w, h) = (s.x as f32, s.y as f32, w as f32, h as f32);
        match dir {
            Dir::Up => [x + w / 2.0, y],
            Dir::Down => [x + w / 2.0, y + h],
            Dir::Left => [x, y + h / 2.0],
            Dir::Right => [x + w, y + h / 2.0],
        }
    }
}

impl crate::Content {
    /// The facing of placement `o` if it is a prism that takes the beam of gun `gun` (a blueprint's placements,
    /// placed in `grid`): the prism's `target` picks the gun's part.
    pub fn prism_for(&self, parts: &[crate::Placement], grid: &ShipGrid, gun: usize, o: usize) -> Option<Dir> {
        let def = self.parts.get(&parts[o].part)?;
        let p = def.prism.as_ref()?;
        self.parts.get(&parts[gun].part)?.matches(&p.target).then(|| grid.shape(o).facing())
    }

    /// The beam path of gun `gun` of a blueprint's placements in its intact grid (every part stands in the way):
    /// the drydock's and validation's line of fire (GDR-0126).
    pub fn beam_path(&self, parts: &[crate::Placement], grid: &ShipGrid, gun: usize) -> BeamPath {
        grid.beam_path(gun, |_| true, |o| self.prism_for(parts, grid, gun, o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::model::Shape;
    use crate::design::geometry::{Footprint, PlacedShape};

    fn at(x: i32, y: i32, rot: u8) -> PlacedShape {
        PlacedShape { footprint: Footprint { w: 1, h: 1, shape: Shape::Rect }, x, y, rot, flip: false }
    }

    /// Part 0 is the gun; the parts in `prisms` are prisms (any target).
    fn path(shapes: Vec<PlacedShape>, prisms: &[usize]) -> BeamPath {
        let g = ShipGrid::new(shapes).unwrap();
        g.beam_path(0, |_| true, |o| prisms.contains(&o).then(|| g.shape(o).facing()))
    }

    #[test]
    fn a_straight_chain_turns_and_ends() {
        // A gun at (0, 3) facing up, prisms at (0, 2) and (0, 1) facing up: it leaves from (0, 1).
        let p = path(vec![at(0, 3, 0), at(0, 2, 0), at(0, 1, 0), at(1, 3, 0)], &[1, 2]);
        assert_eq!((p.prisms(), p.exit, p.dir, p.blocker), (&[1u32, 2][..], 2, Dir::Up, None));
        // A prism turned right sends the beam right; one facing back at the gun blocks.
        let p = path(vec![at(0, 3, 0), at(0, 2, 1)], &[1]);
        assert_eq!((p.exit, p.dir, p.clear()), (1, Dir::Right, true));
        let p = path(vec![at(0, 3, 0), at(0, 2, 2)], &[1]);
        assert_eq!((p.len, p.blocker), (0, Some(1)));
        // A part that is no prism blocks; so does one in front of the last prism.
        let p = path(vec![at(0, 3, 0), at(0, 2, 0), at(0, 0, 0)], &[1]);
        assert_eq!((p.len, p.blocker), (1, Some(2)));
    }

    #[test]
    fn a_gap_is_crossed_and_the_chain_is_capped() {
        let p = path(vec![at(0, 9, 0), at(0, 7, 0)], &[1]);
        assert_eq!((p.len, p.exit, p.clear()), (1, 1, true));
        let shapes: Vec<_> = (0..7).map(|k| at(0, 9 - k, 0)).collect();
        let p = path(shapes, &[1, 2, 3, 4, 5, 6]);
        assert_eq!((p.len as usize, p.blocker), (MAX_PRISMS, Some(MAX_PRISMS + 1)));
    }

    #[test]
    fn a_loop_of_prisms_blocks() {
        // Up, right, down, left: back into the first prism.
        let p = path(vec![at(0, 3, 0), at(0, 2, 1), at(1, 2, 2), at(1, 3, 3)], &[1, 2, 3]);
        assert_eq!(p.blocker, Some(0), "the last prism faces the gun");
        let p = path(vec![at(0, 4, 0), at(0, 3, 0), at(0, 2, 1), at(1, 2, 2), at(1, 3, 3)], &[1, 2, 3, 4]);
        assert_eq!((p.len, p.blocker), (4, Some(1)), "met twice: blocked");
    }
}
