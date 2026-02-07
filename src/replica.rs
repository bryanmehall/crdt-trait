/// A trait representing an entity capable of identifying itself in a distributed system.
///
/// A `Replica` is responsible for managing its own identity. It produces an `Id` that can
/// be used to sign updates for Conflict-Free Replicated Data Types (CRDTs).
///
/// This trait abstracts over different identity strategies:
/// - **Static/Random**: E.g., UUIDs where `fork` generates a new random ID.
/// - **Structural/Dynamic**: E.g., Interval Tree Clocks (ITC) where `fork` splits the identity space.
pub trait Replica {
    /// The specific type of identifier this replica produces.
    ///
    /// This ID is passed to CRDT update methods to sign operations or track causality.
    type Id: Clone + PartialEq + std::fmt::Debug;

    /// Returns the current identifier for this replica.
    fn id(&self) -> Self::Id;

    /// Splits this replica into two valid, distinct replicas.
    ///
    /// - For random identifiers (like UUIDs), this simply generates a new independent replica.
    /// - For structural identifiers (like ITC), this partitions the identity space (e.g., splitting an interval).
    fn fork(&mut self) -> Self;

    /// Merges another replica back into this one, consuming it.
    ///
    /// - For random identifiers, this is typically a no-op (the other ID is discarded).
    /// - For structural identifiers, this recombines the identity space (e.g., merging intervals).
    fn join(&mut self, other: Self);
}
