// Fixed-capacity normalized amount vectors. Pool reserves and user-supplied
// amounts both cross math seams in this shape: token order plus a checked count.

use crate::math::MAX_TOKENS;

#[derive(Clone, Copy)]
pub struct NormalizedAmounts {
    values: [u64; MAX_TOKENS],
    len: usize,
}

impl NormalizedAmounts {
    pub fn new(values: [u64; MAX_TOKENS], len: usize) -> Option<Self> {
        if len > MAX_TOKENS {
            return None;
        }
        Some(Self { values, len })
    }

    pub fn as_slice(&self) -> &[u64] {
        &self.values[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn get(&self, index: usize) -> Option<u64> {
        self.as_slice().get(index).copied()
    }

    pub fn contains_zero(&self) -> bool {
        self.as_slice().contains(&0)
    }

    pub fn any_positive(&self) -> bool {
        self.as_slice().iter().any(|amount| *amount > 0)
    }
}
