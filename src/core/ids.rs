//! Small, dependency-free identifiers.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

/// A process-unique identifier.
///
/// Ids are handed out in creation order, so sorting by id is also sorting by
/// age. They only need to be unique within a session: persisted state refers
/// to alarms and world clocks by these ids, and a fresh session allocates
/// ids that cannot collide with a restored one because the counter starts
/// above the highest id seen on load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

impl Id {
    /// Allocates the next identifier.
    ///
    /// Stops at the ceiling rather than wrapping round: a counter that wrapped
    /// would hand out identifiers already in use, and an identifier issued twice
    /// is two things sharing one name.
    pub fn next() -> Id {
        let mut current = NEXT.load(Ordering::Relaxed);
        loop {
            // Saturating, so that a counter already at the ceiling stays there
            // rather than wrapping round to identifiers already in use. The
            // comparison-exchange is the portable spelling of "take the next
            // one"; at the ceiling it writes back the value it read, which is
            // exactly what staying put means.
            let wanted = current.saturating_add(1);
            match NEXT.compare_exchange_weak(current, wanted, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Id(current),
                Err(observed) => current = observed,
            }
        }
    }

    /// Forges an identifier, used when restoring persisted state.
    pub fn from_raw(raw: u64) -> Id {
        // Keep the allocator ahead of anything we load, so a restored id can
        // never be handed out again. Saturating, because a stored document can
        // carry the largest identifier there is and `raw + 1` would overflow a
        // debug build on the way to being discarded; at the ceiling the
        // allocator simply refuses to go higher.
        NEXT.fetch_max(raw.saturating_add(1), Ordering::Relaxed);
        Id(raw)
    }

    /// The raw value, for serialization.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Id;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<Id> = (0..10_000).map(|_| Id::next()).collect();
        assert_eq!(ids.len(), 10_000);
    }

    #[test]
    fn ids_increase_monotonically() {
        let first = Id::next();
        let second = Id::next();
        assert!(second > first);
    }

    #[test]
    fn raw_round_trips() {
        let id = Id::from_raw(4_294_967_296);
        assert_eq!(id.raw(), 4_294_967_296);
        assert_eq!(Id::from_raw(id.raw()), id);
    }

    #[test]
    fn loading_an_id_advances_the_allocator() {
        let restored = Id::from_raw(9_000);
        let next = Id::next();
        assert!(next > restored, "a restored id must never be reused");
    }
}
