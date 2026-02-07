use crdt::{Apply, Crdt};
use std::collections::HashMap;

/// A GCounter implemented using the "Delta State" pattern.
///
/// In this pattern:
/// 1. Every "Event" is actually a mini-CRDT (a Delta).
/// 2. The `Apply` implementation simply delegates to `merge`.
/// 3. Local operations update the state AND return the Delta to be shipped.
#[derive(Debug, Clone, PartialEq, Default)]
struct DeltaGCounter {
    counts: HashMap<String, u64>,
}

// The "Event" is just a Delta - a partial state containing only the changes.
type CounterEvent = DeltaGCounter;

impl Crdt for DeltaGCounter {
    type Value = u64;

    fn merge(&mut self, other: &Self) {
        for (id, &val) in &other.counts {
            let entry = self.counts.entry(id.clone()).or_insert(0);
            *entry = (*entry).max(val);
        }
    }

    fn value(&self) -> Self::Value {
        self.counts.values().sum()
    }
}

// In this pattern, Apply::Op is the State itself (or a Delta of it).
// Apply::apply simply delegates to merge!
impl Apply for DeltaGCounter {
    type Op = CounterEvent;
    type Context = (); // Context is embedded in the Delta (the Key/Value)

    fn apply(&mut self, op: Self::Op, _ctx: Self::Context) {
        self.merge(&op);
    }
}

impl DeltaGCounter {
    // Ergonomic Local Mutator
    // 1. Calculates the change based on current state.
    // 2. Creates a Delta (Event).
    // 3. Applies it locally.
    // 4. Returns the Delta for replication.
    pub fn inc(&mut self, replica_id: impl Into<String>) -> CounterEvent {
        let id = replica_id.into();

        // Calculate next value based on CURRENT state
        let current_val = *self.counts.get(&id).unwrap_or(&0);
        let new_val = current_val + 1;

        // Create the Delta (The Event)
        let mut delta = DeltaGCounter::default();
        delta.counts.insert(id.clone(), new_val);

        // Apply locally (Merge the delta)
        self.merge(&delta);

        // Return the delta for distribution
        delta
    }
}

fn main() {
    println!("--- Delta State Pattern Example ---");
    println!("Demonstrating how 'Events' can be state Deltas for free idempotence.\n");

    let mut replica_a = DeltaGCounter::default();
    let mut replica_b = DeltaGCounter::default();

    println!("1. Replica A performs an action.");
    // Ergonomic usage: looks like a normal method call, but captures the 'Event'
    let event_1 = replica_a.inc("node_a");

    println!("   Replica A State: {:?}", replica_a);
    println!("   Generated Event (Delta): {:?}\n", event_1);

    println!("2. Replica B receives the event.");
    // The 'Apply' trait is used here, but it's really just a merge.
    replica_b.apply(event_1.clone(), ());
    println!("   Replica B State: {:?}\n", replica_b);

    println!("3. Replica B receives the SAME event again (Network Retry).");
    // Because the Event is a State Delta, it is IDEMPOTENT by default.
    // A pure Op-based counter (e.g. "Add 1") would have incorrectly incremented to 2 here.
    replica_b.apply(event_1.clone(), ());
    println!("   Replica B State (After Duplicate): {:?}\n", replica_b);

    assert_eq!(
        replica_b.value(),
        1,
        "Value should remain 1 despite duplicate delivery"
    );

    println!("4. Batching Events");
    // We can merge multiple events into one before sending!
    // This effectively "squashes" the history into a single state update.
    let event_2 = replica_a.inc("node_a"); // Val -> 2
    let event_3 = replica_a.inc("node_a"); // Val -> 3

    let mut batch_event = DeltaGCounter::default();
    batch_event.merge(&event_2);
    batch_event.merge(&event_3);

    println!("   Batched Event (Squashed 2 & 3): {:?}", batch_event);

    replica_b.apply(batch_event, ());
    println!("   Replica B State (After Batch): {:?}", replica_b);

    assert_eq!(replica_b.value(), 3);
}
