//! Replay hashing: hand-rolled FNV-1a-64.
//!
//! `std`'s hashers are process-seeded and `#[derive(Hash)]` depends on field
//! order/layout — both unusable for a hash that must be identical across
//! runs, platforms and versions (determinism contract, `lib.rs` rule 3).
//! Byte order canon: little-endian everywhere.

pub struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    pub fn new() -> Self {
        Fnv1a64(Self::OFFSET_BASIS)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            // wrapping_mul is *deliberate*, not an overflow bug: FNV-1a is
            // defined as multiplication mod 2^64. This is the one place where
            // wrapping is the spec — hence no plain `*` (overflow-checks
            // would rightly panic).
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    pub fn write_u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }

    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    pub fn write_i64(&mut self, v: i64) {
        self.write(&v.to_le_bytes());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known FNV-1a test vectors — guards against typos in the constants.
    #[test]
    fn known_vectors() {
        assert_eq!(Fnv1a64::new().finish(), 0xcbf2_9ce4_8422_2325);

        let mut h = Fnv1a64::new();
        h.write(b"a");
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);

        let mut h = Fnv1a64::new();
        h.write(b"foobar");
        assert_eq!(h.finish(), 0x85944171f73967e8);
    }
}
