//! What the two round trip tests share: a seeded random number generator and the merge of two JSON objects.

use serde_json::Value;

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub fn pick<'a, T>(&mut self, of: &'a [T]) -> &'a T {
        &of[self.below(of.len() as u64) as usize]
    }

    pub fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

/// Put the members of `b` into `a` (both are objects).
pub fn merge(a: &mut Value, b: Value) {
    a.as_object_mut()
        .unwrap()
        .extend(b.as_object().unwrap().clone());
}
