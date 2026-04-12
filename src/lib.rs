#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod causal;
pub mod crdt;
pub mod delta_sync;
pub mod identified;
pub mod primitive;
pub mod replica;
pub mod traits;

pub use crate::causal::itc::{ItcClock, ItcId, ItcReplica};
#[cfg(feature = "std")]
pub use crate::causal::vector::VectorClock;
pub use crate::crdt::Crdt;
#[cfg(feature = "std")]
pub use crate::identified::gcounter::GCounter;
#[cfg(feature = "std")]
pub use crate::primitive::gset::GSet;
pub use crate::replica::Replica;
pub use crate::causal::Causal;
pub use crate::delta_sync::DeltaSync;
pub use crate::identified::Identified;
pub use crate::traits::Apply;

#[cfg(feature = "derive")]
pub use crdt_derive::Crdt;
#[cfg(feature = "derive")]
pub use crdt_derive::DeltaSync;

#[cfg(feature = "proptest")]
pub mod properties {
    pub use crate::crdt::checks::*;
    pub use crate::delta_sync::checks::*;
}
