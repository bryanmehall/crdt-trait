use super::Crdt;
use pretty_assertions::Comparison;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestError, TestRunner};
use std::fmt::Debug;

/// Private helper to handle test results and provide clean error messages.
fn handle_test_result<T: Debug>(result: Result<(), TestError<T>>, input_labels: &str) {
    match result {
        Ok(_) => (),
        Err(TestError::Fail(reason, counterexample)) => {
            panic!(
                "\n\n--- CRDT PROPERTY FAILURE ---\n\
                {}\n\n\
                Input values ({}):\n{:#?}\n\
                -----------------------------\n",
                reason, input_labels, counterexample
            );
        }
        Err(err) => panic!("\nCRDT Property Check Error: {:?}\n", err),
    }
}

/// Returns a TestRunner configured for CRDT property checks.
fn create_runner() -> TestRunner {
    TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    })
}

/// Checks that the Idempotence property holds: `A ⊔ A = A`
pub fn check_idempotence<T>()
where
    T: Crdt + Arbitrary,
{
    let mut runner = create_runner();
    let result = runner.run(&(any::<T>()), |a| {
        let mut b = a.clone();
        b.merge(&a);

        if a != b {
            return Err(TestCaseError::fail(format!(
                "Idempotence failed (A ⊔ A != A):\n\
                Legend: < A (Expected) / > A ⊔ A (Actual Result)\n{}",
                Comparison::new(&a, &b)
            )));
        }
        Ok(())
    });
    handle_test_result(result, "A");
}

/// Checks that the Commutativity property holds: `A ⊔ B = B ⊔ A`
pub fn check_commutativity<T>()
where
    T: Crdt + Arbitrary,
{
    let mut runner = create_runner();
    let result = runner.run(&(any::<T>(), any::<T>()), |(a, b)| {
        let mut ab = a.clone();
        ab.merge(&b);

        let mut ba = b.clone();
        ba.merge(&a);

        if ab != ba {
            return Err(TestCaseError::fail(format!(
                "Commutativity failed (A ⊔ B != B ⊔ A):\n\
                Legend: < A ⊔ B (Left result) / > B ⊔ A (Right result)\n{}",
                Comparison::new(&ab, &ba)
            )));
        }
        Ok(())
    });
    handle_test_result(result, "A, B");
}

/// Checks that the Associativity property holds: `(A ⊔ B) ⊔ C = A ⊔ (B ⊔ C)`
pub fn check_associativity<T>()
where
    T: Crdt + Arbitrary,
{
    let mut runner = create_runner();
    let result = runner.run(&(any::<T>(), any::<T>(), any::<T>()), |(a, b, c)| {
        let mut ab_c = a.clone();
        ab_c.merge(&b);
        ab_c.merge(&c);

        let mut bc = b.clone();
        bc.merge(&c);
        let mut a_bc = a.clone();
        a_bc.merge(&bc);

        if ab_c != a_bc {
            return Err(TestCaseError::fail(format!(
                "Associativity failed ((A ⊔ B) ⊔ C != A ⊔ (B ⊔ C)):\n\
                Legend: < (A ⊔ B) ⊔ C (Left result) / > A ⊔ (B ⊔ C) (Right result)\n{}",
                Comparison::new(&ab_c, &a_bc)
            )));
        }
        Ok(())
    });
    handle_test_result(result, "A, B, C");
}

/// Runs all CRDT property checks for type T.
pub fn check_eventual_consistency<T>()
where
    T: Crdt + Arbitrary,
{
    check_idempotence::<T>();
    check_commutativity::<T>();
    check_associativity::<T>();
}
