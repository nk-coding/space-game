//! State hashing: FNV-1a style over 32-bit words (raw float bit patterns), stable and platform independent.

#[derive(Clone, Debug)]
pub struct StateHasher(u64);

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

impl Default for StateHasher {
    fn default() -> Self {
        StateHasher(OFFSET)
    }
}

impl StateHasher {
    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.0 = (self.0 ^ v as u64).wrapping_mul(PRIME);
    }
    #[inline]
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.u32(v as u32);
        self.u32((v >> 32) as u32);
    }
    /// One body: position, rotation (cos, sin), linear velocity, angular velocity.
    #[inline]
    pub fn body(&mut self, pos: [f32; 2], rot: [f32; 2], linvel: [f32; 2], angvel: f32) {
        self.f32(pos[0]);
        self.f32(pos[1]);
        self.f32(rot[0]);
        self.f32(rot[1]);
        self.f32(linvel[0]);
        self.f32(linvel[1]);
        self.f32(angvel);
    }
    pub fn finish(&self) -> u64 {
        self.0
    }
}
