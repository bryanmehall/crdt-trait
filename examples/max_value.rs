use crdt::Crdt;
use crdt::properties;
use proptest::prelude::*;

/// A simple CRDT where the state is a single value,
/// and the merge operation takes the maximum of the two values.
#[derive(Debug, Clone, PartialEq, Default)]
struct MyStruct {
    value: u32,
}

impl Crdt for MyStruct {
    fn merge(&mut self, other: &Self) {
        self.value = self.value.max(other.value);
    }
}

// Implement Arbitrary for our type to use the automatic property verification helpers.
impl Arbitrary for MyStruct {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<u32>().prop_map(|value| MyStruct { value }).boxed()
    }
}

fn main() {
    let mut a = MyStruct { value: 10 };
    let b = MyStruct { value: 20 };

    println!("Initial state: a = {:?}, b = {:?}", a, b);

    a.merge(&b);

    println!("After merge: a = {:?}", a);
    assert_eq!(a.value, 20);

    println!("\nRunning automated property tests...");
    properties::check_eventual_consistency::<MyStruct>();
    println!("All CRDT properties (Idempotence, Commutativity, Associativity) hold!");
}
