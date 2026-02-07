use crate::{Apply, Crdt};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::Hash;

/// A Vector Clock CRDT.
///
/// Tracks causality in a distributed system. A Vector Clock is a map of
/// replica IDs to logical timestamps (counters).
///
/// # Type Parameters
/// * `I`: The type of the Replica ID. Must be `Hash`, `Eq`, `Clone`, and `Debug`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct VectorClock<I>
where
    I: Hash + Eq,
{
    clocks: HashMap<I, u64>,
}

impl<I: Hash + Eq> Default for VectorClock<I> {
    fn default() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }
}

impl<I: Hash + Eq + Clone> PartialEq for VectorClock<I> {
    fn eq(&self, other: &Self) -> bool {
        // Two vector clocks are equal if they have the same entries.
        // Missing entries are treated as 0.
        // However, HashMap::eq only checks strictly.
        // We should normalize or check carefully.
        // For simplicity in a CRDT context where we assume strictly growing counters,
        // we usually just delegate to HashMap eq if we assume 0s are pruned or explicit.
        // To be safe against "implicit 0 vs explicit 0", we should check generic containment.

        if self.clocks.len() != other.clocks.len() {
            // Optimization: if lengths differ, they might be different,
            // UNLESS the difference is just zeros.
            // Let's do the rigorous check.
            return self.partial_cmp(other) == Some(Ordering::Equal);
        }
        self.clocks == other.clocks
    }
}

impl<I: Hash + Eq + Clone> Eq for VectorClock<I> {}

impl<I> Crdt for VectorClock<I>
where
    I: Hash + Eq + Clone + std::fmt::Debug,
{
    type Value = HashMap<I, u64>;

    fn merge(&mut self, other: &Self) {
        for (replica, &count) in &other.clocks {
            let entry = self.clocks.entry(replica.clone()).or_insert(0);
            *entry = (*entry).max(count);
        }
    }

    fn value(&self) -> Self::Value {
        self.clocks.clone()
    }
}

impl<I> Apply for VectorClock<I>
where
    I: Hash + Eq + Clone + std::fmt::Debug,
{
    type Op = (); // A tick is just an event
    type Context = I; // Who is ticking?

    fn apply(&mut self, _op: Self::Op, ctx: Self::Context) {
        self.inc(ctx);
    }
}

impl<I> VectorClock<I>
where
    I: Hash + Eq + Clone,
{
    /// Creates a new, empty Vector Clock.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increments the clock for the given replica.
    pub fn inc(&mut self, replica: I) {
        *self.clocks.entry(replica).or_insert(0) += 1;
    }

    /// Returns the logical time for a specific replica.
    pub fn get(&self, replica: &I) -> u64 {
        *self.clocks.get(replica).unwrap_or(&0)
    }

    /// Returns true if this vector clock is strictly causally before the other.
    pub fn happened_before(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Less)
    }

    /// Returns true if this vector clock is concurrent to the other.
    pub fn concurrent(&self, other: &Self) -> bool {
        self.partial_cmp(other).is_none()
    }
}

// PartialOrd implementation for Causality
impl<I> PartialOrd for VectorClock<I>
where
    I: Hash + Eq + Clone,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_is_bigger = false;
        let mut other_is_bigger = false;

        // Check all keys in self
        for (replica, &val) in &self.clocks {
            let other_val = other.get(replica);
            if val > other_val {
                self_is_bigger = true;
            } else if val < other_val {
                other_is_bigger = true;
            }
        }

        // Check keys in other that might be missing in self
        for (replica, &val) in &other.clocks {
            if !self.clocks.contains_key(replica) {
                // self has 0, other has val
                if val > 0 {
                    other_is_bigger = true;
                }
            }
        }

        if self_is_bigger && other_is_bigger {
            None // Concurrent
        } else if self_is_bigger {
            Some(Ordering::Greater)
        } else if other_is_bigger {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Equal)
        }
    }
}
