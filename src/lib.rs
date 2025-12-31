use std::fmt::Debug;

/// The core trait for Conflict-Free Replicated Datatypes.
///
/// Implementors must satisfy the following properties for the `merge` operation:
/// 1. Idempotence: x.merge(x) must result in the same state as x.
/// 2. Commutativity: x.merge(y) must result in the same state as y.merge(x).
/// 3. Associativity: x.merge(y).merge(z) must result in the same state as x.merge(y.merge(z)).
///
/// When the `proptest` feature is enabled, a `properties` module is available
/// containing helper functions to verify CRDT properties. These functions
/// require the type to implement `proptest::arbitrary::Arbitrary`.
pub trait Crdt: Clone + Debug + PartialEq {
    /// Merges another CRDT into this one.
    fn merge(&mut self, other: &Self);
}

/// Helper module for verifying CRDT properties using proptest.
///
/// These functions are available when the `proptest` feature is enabled.
#[cfg(feature = "proptest")]
pub mod properties {
    use super::Crdt;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;

    /// Checks that the Idempotence property holds: `A ⊔ A = A`
    pub fn check_idempotence<T>()
    where
        T: Crdt + Arbitrary,
    {
        let mut runner = TestRunner::default();
        runner
            .run(&(any::<T>()), |a| {
                let mut b = a.clone();
                b.merge(&a);

                if a != b {
                    assert_eq!(a, b, "Idempotence failed");
                }
                Ok(())
            })
            .unwrap();
    }

    /// Checks that the Commutativity property holds: `A ⊔ B = B ⊔ A`
    pub fn check_commutativity<T>()
    where
        T: Crdt + Arbitrary,
    {
        let mut runner = TestRunner::default();
        runner
            .run(&(any::<T>(), any::<T>()), |(a, b)| {
                let mut ab = a.clone();
                ab.merge(&b);

                let mut ba = b.clone();
                ba.merge(&a);

                if ab != ba {
                    assert_eq!(ab, ba, "Commutativity failed");
                }
                Ok(())
            })
            .unwrap();
    }

    /// Checks that the Associativity property holds: `(A ⊔ B) ⊔ C = A ⊔ (B ⊔ C)`
    pub fn check_associativity<T>()
    where
        T: Crdt + Arbitrary,
    {
        let mut runner = TestRunner::default();
        runner
            .run(&(any::<T>(), any::<T>(), any::<T>()), |(a, b, c)| {
                // (A U B) U C
                let mut ab = a.clone();
                ab.merge(&b);
                let mut ab_c = ab;
                ab_c.merge(&c);

                // A U (B U C)
                let mut bc = b.clone();
                bc.merge(&c);
                let mut a_bc = a.clone();
                a_bc.merge(&bc);

                if ab_c != a_bc {
                    assert_eq!(ab_c, a_bc, "Associativity failed");
                }
                Ok(())
            })
            .unwrap();
    }

    /// Runs all CRDT property checks for type T.
    ///
    /// This is a convenience function to run idempotence, commutativity, and associativity tests.
    pub fn check_eventual_consistency<T>()
    where
        T: Crdt + Arbitrary,
    {
        check_idempotence::<T>();
        check_commutativity::<T>();
        check_associativity::<T>();
    }
}
