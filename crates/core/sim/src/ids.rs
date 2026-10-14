//! Stable entity ids: monotonically increasing integers, never reused, independent of allocation addresses.

/// A ship (one blueprint instance). Its pieces may fly apart, but they keep the ship id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShipId(pub u32);

/// A piece: one connected group of parts = one rigid body for the physics layer. Unique across the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PieceId(pub u32);

/// Hands out ids in increasing order. Part of the hashed world state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdAlloc {
    next_ship: u32,
    next_piece: u32,
    next_projectile: u32,
}

impl IdAlloc {
    pub fn ship(&mut self) -> ShipId {
        let id = ShipId(self.next_ship);
        self.next_ship += 1;
        id
    }

    pub fn piece(&mut self) -> PieceId {
        let id = PieceId(self.next_piece);
        self.next_piece += 1;
        id
    }

    pub fn projectile(&mut self) -> crate::combat::weapons::ProjectileId {
        let id = crate::combat::weapons::ProjectileId(self.next_projectile);
        self.next_projectile = self.next_projectile.wrapping_add(1);
        id
    }

    /// `(next ship id, next piece id, next projectile id)`, for hashing and saving.
    pub fn raw(&self) -> (u32, u32, u32) {
        (self.next_ship, self.next_piece, self.next_projectile)
    }
}
