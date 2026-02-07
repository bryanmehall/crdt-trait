/// A trait for Conflict-Free Replicated Data Types that support updates via operations.
///
/// While `Crdt` handles merging state, `Apply` (Commutative Replicated Data Type)
/// defines how to apply local operations to that state.
///
/// Separating the update logic (`Apply`) from the merge logic (`Crdt`) allows
/// composite structs to implement `Crdt` (merging all fields) without needing to
/// define a complex enum of all possible operations for `Apply`.
pub trait Apply {
    /// The operation to apply (e.g., `i32` for a counter increment, or `T` for set insertion).
    type Op;

    /// The context required to apply the operation.
    ///
    /// - For anonymous CRDTs (like G-Set), this might be `()`.
    /// - For identified CRDTs (like G-Counter), this might be `&I` (Replica ID).
    /// - For causal CRDTs, this might include clocks or timestamps.
    type Context;

    /// Applies an operation to the CRDT.
    fn apply(&mut self, op: Self::Op, ctx: Self::Context);
}
