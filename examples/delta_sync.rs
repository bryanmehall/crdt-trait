use crdt::{Crdt, DeltaSync, GCounter, GSet, ItcClock, VectorClock};
use crdt::properties;
use proptest::prelude::*;

fn main() {
    println!("--- Delta Sync Protocol Example ---\n");

    // Create two replicas that diverge
    let mut replica_a = GCounter::new();
    let mut replica_b = GCounter::new();

    // Both replicas see some shared history
    replica_a.add(10, "alice".to_string());
    replica_b.add(10, "alice".to_string());

    // Then they diverge
    replica_a.add(5, "alice".to_string());  // alice: 15 on A
    replica_a.add(3, "bob".to_string());    // bob: 3 on A

    replica_b.add(7, "carol".to_string());  // carol: 7 on B

    println!("Replica A: {:?} (value={})", replica_a.summary(), replica_a.value());
    println!("Replica B: {:?} (value={})", replica_b.summary(), replica_b.value());

    // --- Sync Protocol ---
    // Step 1: B sends its summary to A
    let b_summary = replica_b.summary();
    println!("\n1. B sends summary: {:?}", b_summary);

    // Step 2: A computes delta (what B is missing)
    let delta_for_b = replica_a.delta_from_summary(&b_summary);
    println!("2. A computes delta for B: {:?} (value={})", delta_for_b.summary(), delta_for_b.value());

    // Step 3: B applies the delta
    replica_b.merge_delta(&delta_for_b);
    println!("3. B after applying delta: {:?} (value={})", replica_b.summary(), replica_b.value());

    // Verify: B should now have everything A had, plus its own state
    let mut expected = GCounter::new();
    expected.add(15, "alice".to_string());
    expected.add(3, "bob".to_string());
    expected.add(7, "carol".to_string());
    assert_eq!(replica_b.value(), expected.value());

    // Now sync the other direction
    let a_summary = replica_a.summary();
    let delta_for_a = replica_b.delta_from_summary(&a_summary);
    replica_a.merge_delta(&delta_for_a);

    println!("4. A after reverse sync: {:?} (value={})", replica_a.summary(), replica_a.value());
    assert_eq!(replica_a, replica_b);
    println!("\nBoth replicas converged!\n");

    // --- Property Tests ---
    println!("Running delta sync property tests for GCounter...");
    properties::check_delta_sync_properties::<GCounter<String>>();
    println!("Running delta sync property tests for GSet...");
    properties::check_delta_sync_properties::<GSet<String>>();
    println!("Running delta sync property tests for VectorClock...");
    properties::check_delta_sync_properties::<VectorClock<String>>();
    println!("Running delta sync property tests for ItcClock...");
    properties::check_delta_sync_properties::<ItcClock>();
    println!("All delta sync properties verified!");
}

#[test]
fn test_gcounter_delta_sync_properties() {
    properties::check_delta_sync_properties::<GCounter<String>>();
}

#[test]
fn test_gset_delta_sync_properties() {
    properties::check_delta_sync_properties::<GSet<String>>();
}

#[test]
fn test_vector_clock_delta_sync_properties() {
    properties::check_delta_sync_properties::<VectorClock<String>>();
}

#[test]
fn test_itc_clock_delta_sync_properties() {
    properties::check_delta_sync_properties::<ItcClock>();
}

#[test]
fn test_delta_only_sends_missing() {
    let mut a = GCounter::new();
    a.add(10, "x".to_string());
    a.add(5, "y".to_string());

    let mut b = GCounter::new();
    b.add(10, "x".to_string()); // B already has x=10

    // Delta should only contain y=5, not x=10
    let delta = a.delta_from_summary(&b.summary());
    assert_eq!(delta.value(), 5); // only the y=5 entry

    b.merge_delta(&delta);
    assert_eq!(a, b);
}

// --- Derive macro test ---

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Default, crdt::Crdt, crdt::DeltaSync)]
struct GameState {
    scores: GCounter<String>,
    inventory: GCounter<String>,
}

impl Arbitrary for GameState {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (any::<GCounter<String>>(), any::<GCounter<String>>())
            .prop_map(|(scores, inventory)| GameState { scores, inventory })
            .boxed()
    }
}

#[test]
fn test_derived_delta_sync_properties() {
    properties::check_delta_sync_properties::<GameState>();
}

#[test]
fn test_derived_delta_sync_correctness() {
    let mut a = GameState::default();
    a.scores.add(10, "alice".to_string());
    a.inventory.add(5, "alice".to_string());

    let mut b = GameState::default();
    b.scores.add(10, "alice".to_string()); // same scores
    b.inventory.add(3, "bob".to_string()); // different inventory

    // Delta from A's perspective given B's summary
    let b_summary = b.summary();
    let delta = a.delta_from_summary(&b_summary);

    // Delta should only contain inventory diff (alice: 5), not scores
    b.merge_delta(&delta);

    let mut expected = GameState::default();
    expected.scores.add(10, "alice".to_string());
    expected.inventory.add(5, "alice".to_string());
    expected.inventory.add(3, "bob".to_string());

    assert_eq!(b.scores, expected.scores);
    assert_eq!(b.inventory, expected.inventory);
}

#[test]
fn test_empty_delta_when_in_sync() {
    let mut a = GCounter::new();
    a.add(10, "x".to_string());

    let b = a.clone();

    let delta = a.delta_from_summary(&b.summary());
    assert_eq!(delta.value(), 0); // nothing missing
}
