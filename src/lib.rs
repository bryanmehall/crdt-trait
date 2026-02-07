pub mod causal;
pub mod crdt;
pub mod identified;
pub mod primitive;
pub mod replica;
pub mod traits;

pub use crate::causal::itc::{ItcClock, ItcId, ItcReplica};
pub use crate::causal::vector::VectorClock;
pub use crate::crdt::Crdt;
pub use crate::identified::gcounter::GCounter;
pub use crate::primitive::gset::GSet;
pub use crate::replica::Replica;
pub use crate::traits::Apply;

#[cfg(feature = "derive")]
pub use crdt_derive::Crdt;

#[cfg(feature = "proptest")]
pub mod properties {
    pub use crate::crdt::checks::*;
}
