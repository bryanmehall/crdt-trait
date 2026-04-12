use super::DeltaSync;
use crate::crdt::checks::{check_eventual_consistency, create_runner, handle_test_result};
use crate::Crdt;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// Checks that delta sync produces the same result as full-state merge.
/// This is the fundamental correctness property of delta synchronization.
///
/// For states A and B: A ⊔ δ(B, σ(A)) = A ⊔ B
pub fn check_delta_merge_equivalence<T>()
where
    T: DeltaSync + Arbitrary,
{
    let mut runner = create_runner();
    let result = runner.run(&(any::<T>(), any::<T>()), |(a, b)| {
        // Full-state merge: A ⊔ B
        let mut full_merge = a.clone();
        full_merge.merge(&b);

        // Delta sync: A ⊔ δ(B, σ(A))
        let a_summary = a.summary();
        let delta = b.delta_from_summary(&a_summary);
        let mut delta_merge = a.clone();
        delta_merge.merge_delta(&delta);

        if full_merge != delta_merge {
            return Err(TestCaseError::fail(
                "Delta-Merge Equivalence failed: A ⊔ B != A ⊔ δ(B, σ(A))",
            ));
        }
        Ok(())
    });
    handle_test_result(result, "A, B");
}

/// Checks that merging a delta never removes information.
/// Equivalently: the result of applying a delta subsumes the original state.
///
/// For state A, delta d: (A ⊔ d) ⊔ A = A ⊔ d
pub fn check_delta_inflation<T>()
where
    T: DeltaSync + Arbitrary,
{
    let mut runner = create_runner();
    let result = runner.run(&(any::<T>(), any::<T>()), |(a, b)| {
        let delta = b.delta_from_summary(&a.summary());
        let mut with_delta = a.clone();
        with_delta.merge_delta(&delta);

        // Merging the original back in should be a no-op (delta only added info)
        let mut check = with_delta.clone();
        check.merge(&a);

        if check != with_delta {
            return Err(TestCaseError::fail(
                "Delta Inflation failed: (A ⊔ d) ⊔ A != A ⊔ d",
            ));
        }
        Ok(())
    });
    handle_test_result(result, "A, B");
}

/// Checks that composing (batching) deltas is equivalent to applying them one at a time.
///
/// For state A, deltas d1 and d2: A ⊔ d1 ⊔ d2 = A ⊔ (d1 ⊔ d2)
pub fn check_delta_composition<T>()
where
    T: DeltaSync + Arbitrary,
{
    let mut runner = create_runner();
    let result = runner.run(&(any::<T>(), any::<T>(), any::<T>()), |(a, b, c)| {
        let delta_b = b.delta_from_summary(&a.summary());
        let delta_c = c.delta_from_summary(&a.summary());

        // Apply individually: A ⊔ d_b ⊔ d_c
        let mut individual = a.clone();
        individual.merge_delta(&delta_b);
        individual.merge_delta(&delta_c);

        // Batch: compose deltas first, then apply: A ⊔ (d_b ⊔ d_c)
        let mut batch = delta_b.clone();
        batch.merge(&delta_c);
        let mut batched = a.clone();
        batched.merge_delta(&batch);

        if individual != batched {
            return Err(TestCaseError::fail(
                "Delta Composition failed: A ⊔ d1 ⊔ d2 != A ⊔ (d1 ⊔ d2)",
            ));
        }
        Ok(())
    });
    handle_test_result(result, "A, B, C");
}

/// Runs all DeltaSync property checks for type T.
/// Also runs the base Crdt eventual consistency checks.
pub fn check_delta_sync_properties<T>()
where
    T: DeltaSync + Arbitrary,
{
    check_eventual_consistency::<T>();
    check_delta_merge_equivalence::<T>();
    check_delta_inflation::<T>();
    check_delta_composition::<T>();
}
