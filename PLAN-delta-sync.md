# Plan: Delta-State Sync for CRDT Trait

## Summary

Add lightweight delta-state synchronization to the library so that replicas can exchange minimal state fragments instead of full states. This builds on the existing `Crdt` trait and library organization (Primitive, Identified, Causal) by introducing a `DeltaSync` trait that formalizes each category's ability to produce compact sync summaries and compute optimal deltas.

## Motivation

The current `Crdt::merge` requires shipping the **full state** between replicas. For large CRDTs (e.g., a GCounter with thousands of replicas, or a GSet with millions of elements) this is wasteful — most of the state is already shared. Delta-state CRDTs solve this by only sending the parts that differ.

The key challenge is: **how does Replica A know what Replica B is missing without seeing B's full state?** The answer depends on the CRDT category:

| Category | Compact Summary | Size |
|----------|----------------|------|
| Primitive (GSet) | None — no identity to track | O(data) |
| Identified (GCounter, VectorClock) | Version vector: `Map<ReplicaId, u64>` | O(replicas) |
| Causal (ITC) | Causal context (dot set / version vector) | O(replicas) |

This means the library's existing Primitive/Identified/Causal classification isn't just organizational — it determines what kind of efficient sync is possible. Making this a trait hierarchy formalizes that.

## References

| Paper | Year | Key Contribution |
|-------|------|-----------------|
| [Efficient State-based CRDTs by Delta-Mutation](https://arxiv.org/abs/1410.2803) (Almeida, Shoker, Baquero) | 2015 | Introduces δ-CRDTs: delta-mutators that return state fragments instead of full states |
| [Delta State Replicated Data Types](https://arxiv.org/abs/1603.01529) (Almeida, Baquero, Shoker) | 2016 | Anti-entropy algorithms for convergence (Alg 1) and causal consistency (Alg 2) |
| [Efficient Synchronization of State-based CRDTs](https://arxiv.org/abs/1803.02750) (Enes, Almeida, Baquero, Leitão) | 2018 | Join decomposition: compute optimal deltas by structurally diffing two states |

## Design

### Trait Hierarchy

```
Crdt                         (merge, value — the three convergence properties)
├── Apply                    (local mutation via operations)
├── Replica                  (identity management — fork/join)
└── DeltaSync                (compact summary → efficient delta computation)
    ├── Identified           (summary = version vector, static replica set)
    └── Causal               (summary = causal context, dynamic replica set)
```

Primitive CRDTs implement `Crdt` + `Apply` only. They work correctly, but don't get efficient delta sync (they'd fall back to full-state merge). If you want efficient sync, you opt into `Identified` or `Causal`, which tells the library (and the derive macro) what kind of summary to generate.

> **Note on `Identified` and `Causal`:** These sub-traits are primarily library-level abstractions — they enable the derive macro to generate correct `DeltaSync` impls for composed structs and allow generic code over sync categories. Most end users will interact with `DeltaSync` directly and won't need to name `Identified` or `Causal` in their own code.

### `no_std` Compatibility

The `DeltaSync` trait definition (and `Identified`/`Causal`) must be `no_std`-compatible — they are pure trait definitions with no allocator dependency. Individual implementations may require `std` (e.g., `GCounter` and `VectorClock` use `HashMap`), and those impls are gated behind `#[cfg(feature = "std")]` as their types already are. The `ItcClock` implementation should remain `no_std` since `ItcClock` itself is `no_std`.

`DeltaSync` is not behind a feature flag — it is always available, zero-cost if unused.

### The `DeltaSync` Trait

```rust
/// A CRDT that supports efficient delta synchronization.
///
/// Instead of shipping the full state, a replica can send a compact `Summary`
/// of what it knows. The receiving replica uses that summary to compute a
/// minimal `Delta` containing only the missing information.
///
/// ## Sync Protocol
///
/// 1. Replica B sends `B.summary()` to Replica A (cheap — much smaller than full state)
/// 2. Replica A computes `A.delta_from_summary(&b_summary)` (only what B is missing)
/// 3. Replica A sends the delta to B
/// 4. Replica B calls `B.merge_delta(&delta)` to apply it
///
/// After step 4, B's state reflects everything A knew at step 2.
pub trait DeltaSync: Crdt {
    /// A compact representation of "what this replica already knows."
    /// Must be much smaller than the full state to be useful.
    ///
    /// - For Identified CRDTs: a version vector `Map<ReplicaId, u64>`
    /// - For Causal CRDTs: a causal context (set of dots)
    type Summary: Clone + Debug;

    /// The delta payload — a state fragment containing only missing information.
    /// Often the same type as `Self`, since deltas live in the same join-semilattice.
    type Delta: Crdt;

    /// Extract a compact summary of the current state.
    fn summary(&self) -> Self::Summary;

    /// Given a remote peer's summary, compute the minimal delta
    /// that would bring them up to date with our state.
    fn delta_from_summary(&self, remote_summary: &Self::Summary) -> Self::Delta;

    /// Merge a delta into this state.
    ///
    /// When `Delta = Self`, this is equivalent to `merge`. There is no blanket impl
    /// for this case — implementations write the forwarding call explicitly, and the
    /// derive macro generates it automatically. This keeps the trait simple and avoids
    /// preventing manual specialization for types where `Delta = Self` but the merge
    /// path could be optimized.
    fn merge_delta(&mut self, delta: &Self::Delta);
}
```

### Identified and Causal as Traits

These traits are primarily library-internal — they inform the derive macro and enable generic library code. End users typically interact with `DeltaSync` directly.

```rust
/// An Identified CRDT uses static replica IDs to partition state.
///
/// Its sync summary is a version vector: one entry per known replica,
/// mapping replica ID to the highest sequence number seen from that replica.
///
/// This is cheap — O(replicas) regardless of how large the actual data is.
///
/// This trait is primarily used by the derive macro to generate correct
/// `DeltaSync` impls for composed structs. End users rarely need to
/// reference it directly.
pub trait Identified: DeltaSync {
    type ReplicaId: Hash + Eq + Clone + Debug;
}

/// A Causal CRDT tracks causality using dynamic, forkable identities.
///
/// Its sync summary is a causal context — a compact representation of
/// all events (dots) this replica has observed.
///
/// For ITC, the summary is derived from the event tree structure itself.
/// This trait is primarily used by the derive macro and generic library code.
pub trait Causal: DeltaSync {
    type Dot: Clone + Debug; // (ReplicaId, SequenceNumber)
}
```

### Concrete Implementation: GCounter

```rust
impl<I: Hash + Eq + Clone + Debug> DeltaSync for GCounter<I> {
    // The summary is a version vector: for each replica, the count we've seen.
    // This IS the state for GCounter, but for richer CRDTs the summary would
    // be much smaller than the data.
    type Summary = HashMap<I, u64>;

    // Deltas are the same type — a partial GCounter containing only changed entries.
    type Delta = Self;

    fn summary(&self) -> Self::Summary {
        self.counts.clone()
    }

    fn delta_from_summary(&self, remote_summary: &Self::Summary) -> Self {
        let mut delta = GCounter::new();
        for (replica, &count) in &self.counts {
            let remote_count = remote_summary.get(replica).copied().unwrap_or(0);
            if count > remote_count {
                // Remote is behind on this replica — include it in the delta.
                delta.counts.insert(replica.clone(), count);
            }
        }
        delta
    }

    fn merge_delta(&mut self, delta: &Self) {
        self.merge(delta);
    }
}
```

For GCounter specifically the summary happens to be the same shape as the state — the version vector *is* the data. But the summary is sent as lightweight metadata (just the version vector), not the data payload itself. For a richer CRDT (e.g., a map of ReplicaId to large documents), the summary would still be O(replicas) while the actual data could be arbitrarily large. The version vector travels as sync metadata; the elements do not.

### Derive Macro: Composed Structs

For a composed struct where all fields are `Identified` with the same `ReplicaId`:

```rust
#[derive(Crdt, DeltaSync)]
struct GameState {
    scores: GCounter<PlayerId>,
    inventory: GCounter<PlayerId>,
}
```

The derived `DeltaSync` impl would:

- **Summary**: A product (tuple) of per-field summaries — **not** a merged version vector. Merging summaries across fields loses information: if `scores` has `{A: 5, B: 3}` and `inventory` has `{A: 3, B: 7}`, a merged `{A: 5, B: 7}` would cause under-sending for some fields and over-sending for others. Each field needs its own summary to compute its own delta correctly.
- **Delta**: A product struct of per-field deltas.
- **delta_from_summary**: Destructure the summary tuple, call `delta_from_summary` on each field with its corresponding summary, combine into the delta product.
- **merge_delta**: Call `merge_delta` on each field with its corresponding delta.

For structs mixing Identified and Primitive fields, the derive macro would either:
1. **Refuse to derive** `DeltaSync` (safe default — user must implement manually), or
2. Include the Primitive fields in full in every delta (correct but not minimal).

### Properties to Proptest

All properties are verified using property-based testing via `proptest`, following the existing pattern in `src/crdt/checks.rs`.

#### Delta-Merge Equivalence (the core δ-CRDT contract)

_Syncing via delta must produce the same result as merging full states._

For states $A$ and $B$, summary function $\sigma$, delta function $\delta_\sigma$, and merge operator $\sqcup$:

$$A \sqcup \delta_\sigma(B, \sigma(A)) = A \sqcup B$$

```rust
/// Checks that delta sync produces the same result as full-state merge.
/// This is the fundamental correctness property of delta synchronization.
pub fn check_delta_merge_equivalence<T>()
where
    T: DeltaSync + Arbitrary,
    T::Delta: Debug + PartialEq,
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
                "Delta-Merge Equivalence failed: A ⊔ B != A ⊔ δ(B, σ(A))"
            ));
        }
        Ok(())
    });
    handle_test_result(result, "A, B");
}
```

#### Delta Inflation (monotonicity)

_Merging a delta must never decrease a state — it can only add information._

For state $A$, delta $d$, and merge operator $\sqcup$:

$$(A \sqcup d) \sqcup A = A \sqcup d$$

```rust
/// Checks that merging a delta never removes information.
/// Equivalently: the result of applying a delta subsumes the original state.
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
                "Delta Inflation failed: (A ⊔ d) ⊔ A != A ⊔ d"
            ));
        }
        Ok(())
    });
    handle_test_result(result, "A, B");
}
```

#### Delta Composition (batching)

_Joining multiple deltas into a group and merging the group must equal merging them individually._

For state $A$, deltas $d_1$ and $d_2$, and merge operator $\sqcup$:

$$A \sqcup d_1 \sqcup d_2 = A \sqcup (d_1 \sqcup d_2)$$

This is associativity applied to deltas, but worth testing explicitly because it validates that delta batching is correct.

```rust
/// Checks that composing (batching) deltas is equivalent to applying them one at a time.
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
                "Delta Composition failed: A ⊔ d1 ⊔ d2 != A ⊔ (d1 ⊔ d2)"
            ));
        }
        Ok(())
    });
    handle_test_result(result, "A, B, C");
}
```

#### Combined Check

```rust
/// Runs all DeltaSync property checks for type T.
pub fn check_delta_sync_properties<T>()
where
    T: DeltaSync + Arbitrary,
    T::Delta: Debug + PartialEq + Arbitrary,
{
    check_eventual_consistency::<T>();  // existing Crdt properties still hold
    check_delta_merge_equivalence::<T>();
    check_delta_inflation::<T>();
    check_delta_composition::<T>();
}
```

## Implementation Plan

### Phase 1: `DeltaSync` Trait + GCounter Implementation

**Files changed:**
- `src/delta_sync/mod.rs` — new module with the `DeltaSync` trait
- `src/delta_sync/checks.rs` — proptest property checks (behind `proptest` feature)
- `src/identified/gcounter.rs` — implement `DeltaSync` for `GCounter`
- `src/lib.rs` — export new module

**Tests:**
- `check_delta_sync_properties::<GCounter<String>>()`

**Goal:** Validate the trait design with a concrete, well-understood CRDT.

### Phase 2: Implement for Remaining CRDTs

**Files changed:**
- `src/primitive/gset.rs` — `DeltaSync` for `GSet` (Delta = Self, Summary = Self since there's no compact summary)
- `src/causal/vector.rs` — `DeltaSync` for `VectorClock`
- `src/causal/itc.rs` — `DeltaSync` for `ItcClock`

**GSet:** Implement with `Summary = Self`. There's no compact summary without bloom filters or Merkle trees, but the delta payload still saves bandwidth when states overlap (set difference is smaller than either full state). The summary being the full state is honest — it just means GSet gets delta savings only on the response, not the request.

**VectorClock:** Straightforward — `Summary = HashMap<I, u64>` (same shape as state), `Delta = Self`. Same pattern as GCounter.

**ItcClock (`no_std`):** The event tree *is* the causal metadata. ITC doesn't use version vectors — it uses structural tree positions instead of `(ReplicaId, SeqNum)` dots.

- `Summary = EventTree` — the event tree encodes everything this replica has observed. It's the ITC equivalent of a version vector.
- `Delta = ItcClock` — a partial event tree containing only the events the remote hasn't seen.
- `delta_from_summary`: Compare our event tree against the remote's event tree to identify subtrees where we're strictly ahead. Emit a delta containing only those regions. This is analogous to "for each replica, include the count if ours is higher" but operates on tree structure instead of flat entries.
- `merge_delta`: Calls `merge` (since `Delta = Self`).

The key insight is that `EventTree::join` already computes the pointwise max — so the delta only needs to include subtrees where `self.tree` exceeds the remote summary. Subtrees where the remote is equal or ahead can be replaced with `EventTree::zero()` in the delta.

### Phase 3: `Identified` and `Causal` Traits

**Files changed:**
- `src/identified/mod.rs` — `Identified` trait definition
- `src/causal/mod.rs` — `Causal` trait definition
- Implementations for existing types

**Goal:** Formalize the category distinctions as trait bounds. This enables generic code that works over "any CRDT with a version vector summary" etc.

### Phase 4: Derive Macro Support

**Files changed:**
- `crdt-derive/src/lib.rs` — add `#[derive(DeltaSync)]`

**Rules:**
- All fields must implement `DeltaSync`.
- Summary = product (tuple) of per-field summaries. Each field retains its own summary for correct per-field delta computation.
- Delta = product struct of per-field deltas.
- `delta_from_summary` destructures the summary tuple and calls each field's implementation with its corresponding summary.
- `merge_delta` calls each field's `merge_delta` with its corresponding delta.
- When `Delta = Self` for a field, the generated `merge_delta` forwards to `merge`.

### Phase 5: Example + Documentation

**Files:**
- `examples/delta_sync.rs` — demonstrates the sync protocol with two replicas
- `README.md` — add Delta Sync section explaining the summary-based protocol
- Update the "Choosing a CRDT" flowchart to include sync efficiency

## Resolved Questions

1. **Should `DeltaSync::Delta` default to `Self`?** Not via associated type defaults (unstable). It's the common case and implementations just write `type Delta = Self;` explicitly. Not worth a sub-trait.

2. **GSet's summary problem.** Implement with `Summary = Self`. Honest about the cost — no summary savings, but delta payloads still save bandwidth when states overlap.

3. **Mixed-category composition.** The derive macro either refuses (safe default) or includes Primitive fields in full in every delta (correct but not minimal). User can always implement manually.

4. **Merge_delta vs merge.** No blanket impl. Implementations write the forwarding call explicitly; the derive macro generates it. Keeps the trait simple, doesn't prevent specialization.

5. **Feature flag.** No feature flag. `DeltaSync` is always available, zero-cost if unused.

## Open Questions

1. **ITC delta efficiency.** The event tree diff for `delta_from_summary` needs careful implementation — naively walking the tree and zeroing out regions where the remote is ahead is correct but may produce large deltas when trees have different shapes. Worth benchmarking after Phase 2 to see if normalization or tree restructuring helps.

2. **Derive macro for mixed `Identified` + `Causal` fields.** Should the macro reject this (different summary types can't be meaningfully combined), or is there a useful product-of-summaries interpretation? Likely reject — this combination is unusual and a manual impl would be clearer.