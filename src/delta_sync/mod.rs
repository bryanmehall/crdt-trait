use crate::Crdt;
use core::fmt::Debug;

#[cfg(feature = "proptest")]
pub mod checks;

/// A CRDT that supports efficient delta synchronization.
///
/// Instead of shipping the full state, a replica can send a compact `Summary`
/// of what it knows. The receiving replica uses that summary to compute a
/// minimal `Delta` containing only the missing information.
///
/// ## Sync Protocol
///
/// 1. Replica B sends `B.summary()` to Replica A (cheap — much smaller than full state)
/// 2. Replica A computes `A.delta_from_summary(&b_summary)` (only what B is missing)
/// 3. Replica A sends the delta to B
/// 4. Replica B calls `B.merge_delta(&delta)` to apply it
///
/// After step 4, B's state reflects everything A knew at step 2.
pub trait DeltaSync: Crdt {
    /// A compact representation of "what this replica already knows."
    ///
    /// - For Identified CRDTs: a version vector `Map<ReplicaId, u64>`
    /// - For Causal CRDTs: a causal context (event tree, dot set, etc.)
    /// - For Primitive CRDTs: the full state (no compact summary exists)
    type Summary: Clone + Debug;

    /// The delta payload — a state fragment containing only missing information.
    /// Often the same type as `Self`, since deltas live in the same join-semilattice.
    type Delta: Crdt;

    /// Extract a compact summary of the current state.
    fn summary(&self) -> Self::Summary;

    /// Given a remote peer's summary, compute the minimal delta
    /// that would bring them up to date with our state.
    fn delta_from_summary(&self, remote_summary: &Self::Summary) -> Self::Delta;

    /// Merge a delta into this state.
    ///
    /// When `Delta = Self`, this is equivalent to `merge`. There is no blanket impl
    /// for this case — implementations write the forwarding call explicitly, and the
    /// derive macro generates it automatically. This keeps the trait simple and avoids
    /// preventing manual specialization for types where `Delta = Self` but the merge
    /// path could be optimized.
    fn merge_delta(&mut self, delta: &Self::Delta);
}
