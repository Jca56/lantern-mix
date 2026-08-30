//! Call-site identity via #[track_caller] + scope hashing.

use std::panic::Location;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Id(pub u64);

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

impl Id {
    /// Identity of a widget from where it was called and the enclosing scope.
    pub fn from_location(loc: &Location<'_>, scope: u64) -> Id {
        let h = fnv(FNV_OFFSET, loc.file().as_bytes());
        let h = fnv(h, &loc.line().to_le_bytes());
        let h = fnv(h, &loc.column().to_le_bytes());
        Id(fnv(h, &scope.to_le_bytes()))
    }

    /// Derive a child id (loop index, row number, deck number…).
    pub fn with(self, salt: u64) -> Id {
        Id(fnv(self.0, &salt.to_le_bytes()))
    }

    pub fn from_str(scope: u64, s: &str) -> Id {
        Id(fnv(fnv(FNV_OFFSET, &scope.to_le_bytes()), s.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn here(scope: u64) -> Id {
        Id::from_location(Location::caller(), scope)
    }

    #[test]
    fn different_sites_differ_and_scopes_split() {
        let a = here(0);
        let b = here(0);
        assert_ne!(a, b);
        let c = here(0);
        let d = here(1);
        assert_ne!(c, d);
        assert_ne!(a.with(1), a.with(2));
        assert_eq!(a.with(7), a.with(7));
    }
}
