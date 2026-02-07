use crdt::{Crdt, properties};
use proptest::prelude::*;
use std::collections::HashMap;

/// A simple Grow-only Counter (GCounter) CRDT.
/// It maintains a map of node IDs to their respective counts.
/// The value of the counter is the sum of all counts in the map.
#[derive(Debug, Clone, PartialEq, Default)]
struct GCounter {
    counts: HashMap<String, u64>,
}

impl GCounter {
    /// Increments the counter for a specific node.
    fn increment(&mut self, node_id: &str) {
        let count = self.counts.entry(node_id.to_string()).or_insert(0);
        *count += 1;
    }

    /// Returns the total sum of the counter.
    fn value(&self) -> u64 {
        self.counts.values().sum()
    }
}

impl Crdt for GCounter {
    type Value = u64;

    /// Merges two GCounters by taking the maximum value for each node ID.
    fn merge(&mut self, other: &Self) {
        for (node_id, &other_count) in &other.counts {
            let entry = self.counts.entry(node_id.clone()).or_insert(0);
            *entry = (*entry).max(other_count);
        }
    }

    fn value(&self) -> Self::Value {
        self.value()
    }
}

/// Implement Arbitrary for GCounter to support property-based testing.
impl Arbitrary for GCounter {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // Generate a map with a few node IDs and random counts.
        prop::collection::hash_map(any::<String>(), 0..100u64, 0..5)
            .prop_map(|counts| GCounter { counts })
            .boxed()
    }
}

/// A composite CRDT that uses the `Crdt` derive macro.
/// This struct composes two independent `GCounter` instances.
/// The derive macro ensures that `Stats` is a valid CRDT because all its fields are CRDTs.
#[derive(Debug, Clone, PartialEq, Default, Crdt)]
struct Stats {
    pub visits: GCounter,
    pub errors: GCounter,
}

/// Implement Arbitrary for Stats to support property-based testing.
impl Arbitrary for Stats {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (any::<GCounter>(), any::<GCounter>())
            .prop_map(|(visits, errors)| Stats { visits, errors })
            .boxed()
    }
}

fn main() {
    println!("--- GCounter Composition Example ---");

    let mut node_a_stats = Stats::default();
    let mut node_b_stats = Stats::default();

    // Node A records some visits and an error
    node_a_stats.visits.increment("node_a");
    node_a_stats.visits.increment("node_a");
    node_a_stats.errors.increment("node_a");

    // Node B records a visit
    node_b_stats.visits.increment("node_b");

    println!("Initial state:");
    println!("  Node A Stats: {:?}", node_a_stats);
    println!("  Node B Stats: {:?}", node_b_stats);

    // Merge Node B's state into Node A
    node_a_stats.merge(&node_b_stats);

    println!("\nAfter merge (Node A + Node B):");
    println!("  Merged Stats: {:?}", node_a_stats);
    println!("  Total Visits: {}", node_a_stats.visits.value());
    println!("  Total Errors: {}", node_a_stats.errors.value());

    assert_eq!(node_a_stats.visits.value(), 3);
    assert_eq!(node_a_stats.errors.value(), 1);

    println!("\nRunning automated property tests for composed Stats CRDT...");
    properties::check_eventual_consistency::<Stats>();
    println!("Success: Stats CRDT satisfies all eventual consistency properties!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_eventual_consistency() {
        properties::check_eventual_consistency::<Stats>();
    }
}
