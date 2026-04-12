pub mod itc;
#[cfg(feature = "std")]
pub mod vector;

use crate::DeltaSync;
use core::fmt::Debug;

/// A Causal CRDT tracks causality using dynamic, forkable identities.
///
/// Its sync summary is a causal context — a compact representation of
/// all events (dots) this replica has observed.
///
/// For ITC, the summary is derived from the event tree structure itself.
/// This trait is primarily used by the derive macro and generic library code.
pub trait Causal: DeltaSync {
    type Dot: Clone + Debug;
}
