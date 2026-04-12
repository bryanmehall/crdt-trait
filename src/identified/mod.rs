#[cfg(feature = "std")]
pub mod gcounter;

use crate::DeltaSync;
use core::fmt::Debug;
use core::hash::Hash;

/// An Identified CRDT uses static replica IDs to partition state.
///
/// Its sync summary is a version vector: one entry per known replica,
/// mapping replica ID to the highest sequence number seen from that replica.
///
/// This is cheap — O(replicas) regardless of how large the actual data is.
///
/// This trait is primarily used by the derive macro to generate correct
/// `DeltaSync` impls for composed structs. End users rarely need to
/// reference it directly.
pub trait Identified: DeltaSync {
    type ReplicaId: Hash + Eq + Clone + Debug;
}
