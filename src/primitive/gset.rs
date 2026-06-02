use crate::{Apply, Crdt, DeltaSync};
use std::collections::HashSet;
use std::hash::Hash;

#[cfg(feature = "proptest")]
use proptest::prelude::*;

/// A Grow-only Set (G-Set) CRDT.
///
/// Elements can be added to the set, but never removed.
/// Merging two G-Sets results in the union of their elements.
///
/// # Type Parameters
/// * `T`: The type of elements in the set. Must implement `Hash`, `Eq`, `Clone`, and `Debug`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GSet<T>(HashSet<T>)
where
    T: Hash + Eq;

impl<T: Hash + Eq> Default for GSet<T> {
    fn default() -> Self {
        Self(HashSet::new())
    }
}

impl<T> Crdt for GSet<T>
where
    T: Hash + Eq + Clone + std::fmt::Debug,
{
    type Value = HashSet<T>;

    fn merge(&mut self, other: &Self) {
        // G-Set merge is set union
        for item in &other.0 {
            self.0.insert(item.clone());
        }
    }

    fn value(&self) -> Self::Value {
        self.0.clone()
    }
}

impl<T> Apply for GSet<T>
where
    T: Hash + Eq + Clone + std::fmt::Debug,
{
    type Op = T;
    type Context = ();

    fn apply(&mut self, op: Self::Op, _ctx: Self::Context) {
        self.0.insert(op);
    }
}

impl<T> GSet<T>
where
    T: Hash + Eq,
{
    /// Creates a new, empty G-Set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an element to the set.
    pub fn insert(&mut self, value: T) {
        self.0.insert(value);
    }

    /// Returns true if the set contains the value.
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> DeltaSync for GSet<T>
where
    T: Hash + Eq + Clone + std::fmt::Debug,
{
    // No compact summary exists for a GSet — the full state is the summary.
    type Summary = Self;
    type Delta = Self;

    fn summary(&self) -> Self::Summary {
        self.clone()
    }

    fn delta_from_summary(&self, remote_summary: &Self::Summary) -> Self {
        // Only include elements the remote doesn't have
        let mut delta = GSet::new();
        for item in &self.0 {
            if !remote_summary.0.contains(item) {
                delta.0.insert(item.clone());
            }
        }
        delta
    }

    fn merge_delta(&mut self, delta: &Self) {
        self.merge(delta);
    }
}

#[cfg(feature = "proptest")]
impl Arbitrary for GSet<String> {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        proptest::collection::hash_set("[a-e]".prop_map(String::from), 0..5)
            .prop_map(|set| {
                let mut gset = GSet::new();
                for item in set {
                    gset.insert(item);
                }
                gset
            })
            .boxed()
    }
}
