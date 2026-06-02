//! Delta-state synchronization over a simulated network.
//!
//! Where `delta_sync.rs` calls the `DeltaSync` methods directly on local values,
//! this example forces every sync to go *through the network*: a replica's state
//! is private, and the only way to learn anything about it is to call one of two
//! RPC endpoints. That makes the central constraint of delta sync explicit —
//!
//!   **you cannot compute a delta until you have retrieved the remote's summary.**
//!
//! It also exercises the full range of `Summary` representations:
//!   * `GCounter`     -> version vector            (`HashMap<Id, u64>`)
//!   * `GSet`         -> the full state             (no compact summary exists)
//!   * `ItcClock`     -> a structural `EventTree`
//!   * derived struct -> a *tuple* of per-field summaries, each a different shape
//!
//! Run with: `cargo run --example delta_remote_sync`

use crdt::properties;
use crdt::{Apply, Crdt, DeltaSync, GCounter, GSet, ItcClock, ItcReplica, Replica, VectorClock};
use proptest::prelude::*;
use std::fmt::Debug;

/// A crude proxy for "bytes on the wire": the length of the value's `Debug` form.
/// Good enough to compare delta payloads against full-state payloads.
fn wire_len<X: Debug>(x: &X) -> usize {
    format!("{x:?}").len()
}

/// A replica that lives "on another machine."
///
/// Its CRDT state is private. Peers may only interact through the two endpoints
/// a real sync service would expose:
///   * [`advertise`](Self::advertise) — "here is a summary of what I already know"
///   * [`request_delta`](Self::request_delta) — "given your summary, here is what you're missing"
///
/// Every byte that leaves the node is tallied in `bytes_sent` so we can compare
/// delta traffic against naive full-state replication.
struct RemoteReplica<T: DeltaSync> {
    name: &'static str,
    state: T,
    /// Bytes spent advertising summaries (the version-vector / context leg).
    summary_bytes: usize,
    /// Bytes spent answering with deltas (the actual missing-information leg).
    delta_bytes: usize,
}

impl<T> RemoteReplica<T>
where
    T: DeltaSync + Clone + PartialEq + Debug,
    T::Summary: Debug,
    T::Delta: Debug,
{
    fn new(name: &'static str, state: T) -> Self {
        Self {
            name,
            state,
            summary_bytes: 0,
            delta_bytes: 0,
        }
    }

    /// RPC endpoint #1 — advertise a compact summary of the current state.
    /// This is the cheap message that kicks off every sync.
    fn advertise(&mut self) -> T::Summary {
        let summary = self.state.summary();
        self.summary_bytes += wire_len(&summary);
        summary
    }

    /// RPC endpoint #2 — given a peer's summary, ship only the missing delta.
    fn request_delta(&mut self, peer_summary: &T::Summary) -> T::Delta {
        let delta = self.state.delta_from_summary(peer_summary);
        self.delta_bytes += wire_len(&delta);
        delta
    }

    /// Apply a delta received from a peer.
    fn apply_delta(&mut self, delta: &T::Delta) {
        self.state.merge_delta(delta);
    }

    fn state(&self) -> &T {
        &self.state
    }
}

/// One-directional anti-entropy: bring `target` up to date with `source`,
/// touching `source` only through its public endpoints.
///
/// The ordering here is the whole point: `target` must first advertise its
/// summary, and only *then* can `source` compute the delta. There is no way to
/// skip the round trip — the summary has to come from the remote node.
fn pull_into<T>(target: &mut RemoteReplica<T>, source: &mut RemoteReplica<T>)
where
    T: DeltaSync + Clone + PartialEq + Debug,
    T::Summary: Debug,
    T::Delta: Debug,
{
    let target_summary = target.advertise(); // <- retrieved from the remote (target)
    let delta = source.request_delta(&target_summary); // <- source answers with only the gap
    target.apply_delta(&delta);
}

/// A full bidirectional session between two replicas: each learns from the other.
fn sync_pair<T>(a: &mut RemoteReplica<T>, b: &mut RemoteReplica<T>)
where
    T: DeltaSync + Clone + PartialEq + Debug,
    T::Summary: Debug,
    T::Delta: Debug,
{
    pull_into(a, b);
    pull_into(b, a);
}

fn main() {
    section_1_basic_remote_sync();
    section_2_three_node_gossip();
    section_3_heterogeneous_summary();
    section_4_causal_itc();

    println!("\n--- Property checks (summaries exercised across all CRDT families) ---");
    properties::check_delta_sync_properties::<GCounter<String>>();
    properties::check_delta_sync_properties::<GSet<String>>();
    properties::check_delta_sync_properties::<VectorClock<String>>();
    properties::check_delta_sync_properties::<ItcClock>();
    properties::check_delta_sync_properties::<Document>();
    println!("All delta-sync properties verified, including the derived Document.\n");
}

// ---------------------------------------------------------------------------
// Section 1 — basic: two nodes, one round trip, summary fetched over the wire.
// ---------------------------------------------------------------------------

fn section_1_basic_remote_sync() {
    println!("=== 1. Basic remote sync (GCounter) ===\n");

    let mut a_state = GCounter::new();
    a_state.add(15, "alice".to_string());
    a_state.add(3, "bob".to_string());

    let mut b_state = GCounter::new();
    b_state.add(10, "alice".to_string());
    b_state.add(7, "carol".to_string());

    let mut node_a = RemoteReplica::new("A", a_state);
    let mut node_b = RemoteReplica::new("B", b_state);

    // To bring B up to date we MUST first fetch B's summary, then ask A for the delta.
    let b_summary = node_b.advertise();
    println!("{} advertises summary : {b_summary:?}", node_b.name);

    let delta = node_a.request_delta(&b_summary);
    println!(
        "{} answers with delta : {:?}  (value {})",
        node_a.name,
        delta,
        delta.value()
    );

    node_b.apply_delta(&delta);
    println!("{} after merge_delta  : {:?}", node_b.name, node_b.state());

    // The delta carried alice:15 and bob:3 but never re-sent carol — B already had it.
    assert_eq!(node_b.state().value(), 25);
    println!(
        "B converged to value {} without ever shipping carol back.\n",
        node_b.state().value()
    );
}

// ---------------------------------------------------------------------------
// Section 2 — non-trivial: 3-node ring gossip, measuring bandwidth savings.
// ---------------------------------------------------------------------------

fn section_2_three_node_gossip() {
    println!("=== 2. Three-node ring gossip + bandwidth comparison ===\n");

    let mut a = GCounter::new();
    let mut b = GCounter::new();
    let mut c = GCounter::new();

    // A long shared prefix that all three already agree on. In real systems this
    // dwarfs the recent changes — and it is exactly what delta sync avoids resending.
    for i in 0..20 {
        let id = format!("seed-{i}");
        a.add(100, id.clone());
        b.add(100, id.clone());
        c.add(100, id);
    }

    // Each node then makes one small, independent change.
    a.add(5, "alice".to_string());
    b.add(9, "bob".to_string());
    c.add(2, "carol".to_string());

    let mut na = RemoteReplica::new("A", a);
    let mut nb = RemoteReplica::new("B", b);
    let mut nc = RemoteReplica::new("C", c);

    // Gossip around the ring A<->B, B<->C, C<->A until everyone converges.
    let mut rounds = 0;
    loop {
        rounds += 1;
        sync_pair(&mut na, &mut nb);
        sync_pair(&mut nb, &mut nc);
        sync_pair(&mut nc, &mut na);

        if na.state() == nb.state() && nb.state() == nc.state() {
            break;
        }
        assert!(rounds < 10, "ring gossip failed to converge");
    }

    let converged = na.state().value();
    assert_eq!(converged, nb.state().value());
    assert_eq!(converged, nc.state().value());
    // 20 seed entries * 100 + 5 + 9 + 2.
    assert_eq!(converged, 2016);

    let summary_bytes = na.summary_bytes + nb.summary_bytes + nc.summary_bytes;
    let delta_bytes = na.delta_bytes + nb.delta_bytes + nc.delta_bytes;

    // What a *state-based* CRDT would cost: it has no summary/delta machinery, so
    // every one of the 6 directed syncs per round ships the entire ~21-entry state.
    let full_state_once = wire_len(na.state());
    let naive_bytes = full_state_once * 6 * rounds;

    println!("Converged in {rounds} round(s); all three nodes report value {converged}.\n");
    println!("Bytes a state-based CRDT would gossip : {naive_bytes:>6} (full state, every hop)");
    println!(
        "Delta payloads actually shipped        : {delta_bytes:>6} (only the 3 changed entries)"
    );
    println!("Summary/version-vector overhead        : {summary_bytes:>6} (advertised contexts)");
    println!(
        "\nThe *changed data* was only {:.1}% of a single full-state copy: the 20-entry\n\
         shared prefix was never resent. The dominant cost here is the summary leg —\n\
         a version vector is O(replicas), so for a GCounter it rivals the state itself.\n\
         Delta sync's win grows as the unchanged prefix dwarfs the per-sync changes.\n",
        100.0 * delta_bytes as f64 / full_state_once as f64
    );
    // The genuine delta payloads are a small fraction of even one full-state copy.
    assert!(delta_bytes < full_state_once);
}

// ---------------------------------------------------------------------------
// Section 3 — non-trivial: a derived composite whose Summary is a *tuple* of
// three different summary shapes. Only the field that actually diverged
// produces a non-empty delta.
// ---------------------------------------------------------------------------

/// A collaboratively edited document.
/// Its `Summary` is `(GSet<String>, HashMap<String, u64>, HashMap<String, u64>)` —
/// one entry per field, each a different representation.
#[derive(Debug, Clone, PartialEq, Default, crdt::Crdt, crdt::DeltaSync)]
struct Document {
    /// Set of tags applied to the doc (summary = full state, no compaction).
    tags: GSet<String>,
    /// Per-author edit counts (summary = version vector).
    edits: GCounter<String>,
    /// Causal version of the title field (summary = version vector).
    title_version: VectorClock<String>,
}

impl Arbitrary for Document {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            any::<GSet<String>>(),
            any::<GCounter<String>>(),
            any::<VectorClock<String>>(),
        )
            .prop_map(|(tags, edits, title_version)| Document {
                tags,
                edits,
                title_version,
            })
            .boxed()
    }
}

fn section_3_heterogeneous_summary() {
    println!("=== 3. Derived composite with a heterogeneous tuple summary ===\n");

    // A and B agree on tags and title, but A has one extra edit by "alice".
    let mut a = Document::default();
    a.tags.insert("rust".to_string());
    a.tags.insert("crdt".to_string());
    a.edits.add(4, "alice".to_string());
    a.title_version.inc("alice".to_string());

    let mut b = Document::default();
    b.tags.insert("rust".to_string());
    b.tags.insert("crdt".to_string());
    b.edits.add(2, "alice".to_string()); // B is two edits behind
    b.title_version.inc("alice".to_string());

    let mut node_a = RemoteReplica::new("A", a);
    let mut node_b = RemoteReplica::new("B", b);

    let b_summary = node_b.advertise();
    println!("B's tuple summary:");
    println!("  tags          : {:?}", b_summary.0);
    println!("  edits (vv)    : {:?}", b_summary.1);
    println!("  title_version : {:?}", b_summary.2);

    let delta = node_a.request_delta(&b_summary);
    println!("\nDelta A computes for B (per field):");
    println!(
        "  tags          : {:?}  <- empty, tags already match",
        delta.tags
    );
    println!(
        "  edits         : {:?}  <- only the missing alice edits",
        delta.edits
    );
    println!(
        "  title_version : {:?}  <- empty, title already in sync",
        delta.title_version
    );

    // The tags and title deltas are empty; only `edits` carries information.
    assert!(delta.tags.is_empty());
    assert_eq!(delta.edits.value(), 4); // absolute value alice:4 (delta carries the state)
    assert!(delta.title_version.value().is_empty());

    node_b.apply_delta(&delta);
    assert_eq!(node_b.state().edits.value(), 4);
    assert_eq!(node_b.state(), node_a.state());
    println!("\nB converged; only the diverged `edits` field crossed the wire.\n");
}

// ---------------------------------------------------------------------------
// Section 4 — causal: ITC, whose summary is the structural EventTree.
// ---------------------------------------------------------------------------

fn section_4_causal_itc() {
    println!("=== 4. Causal sync with Interval Tree Clocks ===\n");

    // Fork one seed identity into two independent replicas.
    let mut id_a = ItcReplica::new();
    let id_b = id_a.fork();

    let mut clock_a = ItcClock::default();
    let mut clock_b = ItcClock::default();

    // A ticks three times, B ticks once — they now have divergent event trees.
    for _ in 0..3 {
        clock_a.apply((), id_a.id());
    }
    clock_b.apply((), id_b.id());

    let mut node_a = RemoteReplica::new("A", clock_a);
    let mut node_b = RemoteReplica::new("B", clock_b);

    let a_summary = node_a.advertise();
    println!("A advertises EventTree summary : {a_summary:?}");

    // B answers A's summary with the events A is missing (B's single tick).
    let delta_for_a = node_b.request_delta(&a_summary);
    println!(
        "B's delta for A (missing events): {:?}",
        delta_for_a.value()
    );
    node_a.apply_delta(&delta_for_a);

    // Now sync the other direction so both hold the joined causal history.
    let b_summary = node_b.advertise();
    let delta_for_b = node_a.request_delta(&b_summary);
    node_b.apply_delta(&delta_for_b);

    assert_eq!(node_a.state(), node_b.state());
    println!(
        "Both clocks converged to the joined event tree: {:?}\n",
        node_a.state().value()
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Pulling a delta requires retrieving the remote summary first; afterwards
    /// the puller holds everything the source had, and the source is untouched.
    #[test]
    fn pull_brings_target_up_to_date() {
        let mut source_state = GCounter::new();
        source_state.add(10, "x".to_string());
        source_state.add(5, "y".to_string());

        let mut target_state = GCounter::new();
        target_state.add(10, "x".to_string()); // already has x

        let mut source = RemoteReplica::new("src", source_state);
        let mut target = RemoteReplica::new("tgt", target_state);

        pull_into(&mut target, &mut source);

        assert_eq!(target.state().value(), 15);
        // Source only served data; its own state never changed.
        assert_eq!(source.state().value(), 15);
    }

    /// When two nodes are already in sync, the delta is empty and (for GCounter)
    /// carries no counted value — yet the summary still had to be fetched.
    #[test]
    fn delta_is_empty_when_in_sync() {
        let mut state = GCounter::new();
        state.add(42, "x".to_string());

        let mut a = RemoteReplica::new("a", state.clone());
        let mut b = RemoteReplica::new("b", state);

        let summary = b.advertise();
        assert!(
            b.summary_bytes > 0,
            "advertising a summary costs wire bytes"
        );

        let delta = a.request_delta(&summary);
        assert_eq!(delta.value(), 0, "nothing missing -> empty delta");
        assert!(a.delta_bytes > 0, "even an empty delta is a real message");
    }

    /// A GSet has no compact summary: the summary IS the full state, but the
    /// delta is still minimal (only the elements the peer lacks).
    #[test]
    fn gset_summary_is_full_state_but_delta_is_minimal() {
        let mut a_state = GSet::new();
        for w in ["a", "b", "c", "d"] {
            a_state.insert(w.to_string());
        }
        let mut b_state = GSet::new();
        b_state.insert("a".to_string());
        b_state.insert("b".to_string());

        let mut a = RemoteReplica::new("a", a_state);
        let mut b = RemoteReplica::new("b", b_state);

        let b_summary = b.advertise();
        let delta = a.request_delta(&b_summary);

        // Delta holds only {c, d}, not the elements B already had.
        assert_eq!(delta.len(), 2);
        assert!(delta.contains(&"c".to_string()));
        assert!(delta.contains(&"d".to_string()));

        b.apply_delta(&delta);
        assert_eq!(b.state().len(), 4);
        assert_eq!(a.state(), b.state());
    }

    /// Three nodes with disjoint changes over a shared prefix all converge,
    /// and delta sync moves strictly fewer bytes than full-state broadcast.
    #[test]
    fn ring_gossip_converges_and_saves_bandwidth() {
        let mut a = GCounter::new();
        let mut b = GCounter::new();
        let mut c = GCounter::new();
        for i in 0..15 {
            let id = format!("seed-{i}");
            a.add(50, id.clone());
            b.add(50, id.clone());
            c.add(50, id);
        }
        a.add(1, "a".to_string());
        b.add(2, "b".to_string());
        c.add(3, "c".to_string());

        let mut na = RemoteReplica::new("A", a);
        let mut nb = RemoteReplica::new("B", b);
        let mut nc = RemoteReplica::new("C", c);

        let mut rounds = 0;
        loop {
            rounds += 1;
            sync_pair(&mut na, &mut nb);
            sync_pair(&mut nb, &mut nc);
            sync_pair(&mut nc, &mut na);
            if na.state() == nb.state() && nb.state() == nc.state() {
                break;
            }
            assert!(rounds < 10);
        }

        assert_eq!(na.state().value(), 750 + 6);
        let delta_bytes = na.delta_bytes + nb.delta_bytes + nc.delta_bytes;
        // The genuine delta payloads are smaller than even one full-state copy,
        // despite three nodes gossiping to convergence.
        assert!(delta_bytes < wire_len(na.state()));
    }

    /// The derived composite produces a per-field delta; fields already in sync
    /// contribute empty deltas while the diverged field carries its state.
    #[test]
    fn derived_document_delta_is_per_field() {
        let mut a = Document::default();
        a.tags.insert("shared".to_string());
        a.edits.add(4, "alice".to_string());

        let mut b = Document::default();
        b.tags.insert("shared".to_string());
        b.edits.add(2, "alice".to_string());

        let mut na = RemoteReplica::new("A", a);
        let mut nb = RemoteReplica::new("B", b);

        let b_summary = nb.advertise();
        let delta = na.request_delta(&b_summary);

        assert!(delta.tags.is_empty(), "tags already matched");
        assert_eq!(delta.edits.value(), 4, "edits delta carries alice's state");

        nb.apply_delta(&delta);
        assert_eq!(na.state(), nb.state());
    }

    #[test]
    fn itc_clocks_converge_via_event_tree_summary() {
        let mut id_a = ItcReplica::new();
        let id_b = id_a.fork();
        let mut clock_a = ItcClock::default();
        let mut clock_b = ItcClock::default();
        for _ in 0..3 {
            clock_a.apply((), id_a.id());
        }
        clock_b.apply((), id_b.id());

        let mut na = RemoteReplica::new("A", clock_a);
        let mut nb = RemoteReplica::new("B", clock_b);

        sync_pair(&mut na, &mut nb);
        assert_eq!(na.state(), nb.state());
    }

    #[test]
    fn property_checks_for_derived_document() {
        properties::check_delta_sync_properties::<Document>();
    }
}
