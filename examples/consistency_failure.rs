use crdt::{Crdt, properties};
use proptest::prelude::*;

/// A "broken" CRDT that violates the Commutativity property.
///
/// It simply keeps its own value and ignores the other value during a merge.
/// This means `a.merge(b)` results in `a`, but `b.merge(a)` results in `b`.
/// Since `a.merge(b) != b.merge(a)`, it is not a valid CRDT.
#[derive(Debug, Clone, PartialEq, Default)]
struct BrokenCrdt {
    value: u32,
}

impl Crdt for BrokenCrdt {
    type Value = u32;

    fn merge(&mut self, _other: &Self) {
        // ERROR: We ignore the other value.
        // For a valid Max-Register CRDT, this should be: self.value = self.value.max(_other.value);
    }

    fn value(&self) -> Self::Value {
        self.value
    }
}

// Implement Arbitrary for BrokenCrdt to support property-based testing.
impl Arbitrary for BrokenCrdt {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<u32>().prop_map(|value| BrokenCrdt { value }).boxed()
    }
}

fn main() {
    println!("--- Consistency Failure Example ---");
    println!(
        "This example demonstrates what happens when a data structure violates CRDT properties."
    );
    println!("We have a 'BrokenCrdt' that ignores the 'other' value during a merge.\n");

    let mut a = BrokenCrdt { value: 10 };
    let b = BrokenCrdt { value: 20 };

    println!("Initial state: a = {:?}, b = {:?}", a, b);

    // In a valid CRDT, both of these operations should eventually lead to the same state.
    let mut ab = a.clone();
    ab.merge(&b);

    let mut ba = b.clone();
    ba.merge(&a);

    println!("Result of a.merge(b): {:?}", ab);
    println!("Result of b.merge(a): {:?}", ba);

    if ab != ba {
        println!("\n[!] Commutativity violation detected: a.merge(b) != b.merge(a)");
    }

    println!("\nRunning automated property tests. This is expected to FAIL...");

    // We use std::panic::catch_unwind to capture the failure and show the message
    // without the process exiting immediately with a backtrace.
    let result = std::panic::catch_unwind(|| {
        properties::check_eventual_consistency::<BrokenCrdt>();
    });

    if result.is_err() {
        println!("\nProperty tests FAILED as expected.");
        println!(
            "The proptest suite found a counter-example where the CRDT properties did not hold."
        );
    } else {
        println!("\n[!] Unexpectedly passed! (This shouldn't happen for BrokenCrdt)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Commutativity failed")]
    fn test_broken_crdt_fails() {
        properties::check_commutativity::<BrokenCrdt>();
    }
}
