use crate::{Apply, Crdt};
use std::collections::HashMap;
use std::hash::Hash;

/// A Grow-only Counter (G-Counter) CRDT.
///
/// The counter allows increments, but not decrements. The value of the counter
/// is the sum of the counts from all replicas.
///
/// # Type Parameters
/// * `I`: The type of the Replica ID. Must be `Hash`, `Eq`, `Clone`, and `Debug`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GCounter<I>
where
    I: Hash + Eq,
{
    /// Map of replica IDs to their local counter values.
    counts: HashMap<I, u64>,
    /// Cached sum of all counts to allow O(1) reads.
    cached_value: u64,
}

impl<I: Hash + Eq> Default for GCounter<I> {
    fn default() -> Self {
        Self {
            counts: HashMap::new(),
            cached_value: 0,
        }
    }
}

impl<I> Crdt for GCounter<I>
where
    I: Hash + Eq + Clone + std::fmt::Debug,
{
    type Value = u64;

    fn merge(&mut self, other: &Self) {
        let mut changed = false;
        for (replica, &other_count) in &other.counts {
            let entry = self.counts.entry(replica.clone()).or_insert(0);
            if other_count > *entry {
                *entry = other_count;
                changed = true;
            }
        }

        // If we updated any values, we must recompute the cache.
        if changed {
            self.cached_value = self.counts.values().sum();
        }
    }

    fn value(&self) -> Self::Value {
        self.cached_value
    }
}

impl<I> Apply for GCounter<I>
where
    I: Hash + Eq + Clone + std::fmt::Debug,
{
    type Op = u64;
    type Context = I;

    fn apply(&mut self, op: Self::Op, ctx: Self::Context) {
        self.add(op, ctx);
    }
}

impl<I> GCounter<I>
where
    I: Hash + Eq + Clone,
{
    /// Creates a new GCounter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increments the counter by 1 for the given replica.
    pub fn inc(&mut self, replica: I) {
        self.add(1, replica);
    }

    /// Adds the given amount to the counter for the given replica.
    pub fn add(&mut self, amount: u64, replica: I) {
        let entry = self.counts.entry(replica).or_insert(0);
        *entry += amount;
        self.cached_value += amount;
    }
}
