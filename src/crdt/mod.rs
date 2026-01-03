use std::fmt::Debug;

pub mod checks;

/// The core trait for Conflict-Free Replicated Datatypes.
///
/// Implementors must satisfy the following properties for the `merge` operation:
/// 1. Idempotence: x.merge(x) must result in the same state as x.
/// 2. Commutativity: x.merge(y) must result in the same state as y.merge(x).
/// 3. Associativity: x.merge(y).merge(z) must result in the same state as x.merge(y.merge(z)).
///
/// When the `proptest` feature is enabled, a `properties` module is available
/// containing helper functions to verify CRDT properties. These functions
/// require the type to implement `proptest::arbitrary::Arbitrary`.
pub trait Crdt: Clone + Debug + PartialEq + Default {
    fn init() -> Self {
        Self::default()
    }
    /// Merges another CRDT into this one.
    fn merge(&mut self, other: &Self);
}
