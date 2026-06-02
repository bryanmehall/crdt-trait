# Delta Sync Protocol — Sequence Diagram

This diagram traces the `delta_sync.rs` example step by step, showing the internal
state of each replica and what travels over the wire at each point.

## Scenario

Two GCounter replicas start with shared history, diverge independently, then
synchronize using the delta-state protocol.

---

## Sequence Diagram

```mermaid
sequenceDiagram
    participant A as Replica A
    participant B as Replica B

    Note over A,B: Shared history: both apply add(10, "alice")

    rect rgb(240, 240, 255)
    Note over A,B: Phase 1 — Independent mutations (replicas diverge)
    A->>A: add(5, "alice")
    A->>A: add(3, "bob")
    B->>B: add(7, "carol")
    Note left of A: State: {alice: 15, bob: 3}<br/>Value: 18
    Note right of B: State: {alice: 10, carol: 7}<br/>Value: 17
    end

    rect rgb(240, 255, 240)
    Note over A,B: Phase 2 — Sync B → A (B learns what A knows)

    B->>A: summary() → {alice: 10, carol: 7}
    Note left of A: A compares its state against B's summary:<br/>alice: 15 > 10 ✓ include<br/>bob: 3 > 0 ✓ include<br/>(carol not in A, skip)

    A->>B: delta → {alice: 15, bob: 3}
    Note right of B: B.merge_delta({alice: 15, bob: 3})<br/>alice: max(10, 15) = 15<br/>bob: max(0, 3) = 3<br/>carol: unchanged = 7

    Note right of B: State: {alice: 15, bob: 3, carol: 7}<br/>Value: 25
    Note left of A: State: {alice: 15, bob: 3}<br/>Value: 18 (unchanged)
    end

    rect rgb(255, 245, 230)
    Note over A,B: Phase 3 — Sync A → B (A learns what B knows)

    A->>B: summary() → {alice: 15, bob: 3}
    Note right of B: B compares its state against A's summary:<br/>alice: 15 = 15 ✗ skip<br/>bob: 3 = 3 ✗ skip<br/>carol: 7 > 0 ✓ include

    B->>A: delta → {carol: 7}
    Note left of A: A.merge_delta({carol: 7})<br/>carol: max(0, 7) = 7

    Note left of A: State: {alice: 15, bob: 3, carol: 7}<br/>Value: 25
    Note right of B: State: {alice: 15, bob: 3, carol: 7}<br/>Value: 25
    end

    Note over A,B: ✓ Converged — both replicas identical
```

## State Timeline

```
Time    Replica A                       Replica B
─────   ─────────────────────────       ─────────────────────────
t0      {alice: 10}          = 10       {alice: 10}          = 10
        ↓ add(5, alice)                 ↓ add(7, carol)
        ↓ add(3, bob)
t1      {alice: 15, bob: 3}  = 18       {alice: 10, carol: 7} = 17

                    ── sync B→A ──
                    summary:  {alice: 10, carol: 7}  ←── B sends
                    delta:    {alice: 15, bob: 3}    ──→ A sends

t2      {alice: 15, bob: 3}  = 18       {alice: 15, bob: 3,  = 25
        (unchanged)                       carol: 7}

                    ── sync A→B ──
                    summary:  {alice: 15, bob: 3}    ←── A sends
                    delta:    {carol: 7}             ──→ B sends

t3      {alice: 15, bob: 3,  = 25       {alice: 15, bob: 3,  = 25
          carol: 7}                        carol: 7}
                        ✓ converged
```

## What traveled over the wire

| Step | Direction | Payload | Size | vs Full State |
|------|-----------|---------|------|---------------|
| 1 | B → A | Summary: `{alice: 10, carol: 7}` | 2 entries | Same as full state for GCounter |
| 2 | A → B | Delta: `{alice: 15, bob: 3}` | 2 entries | Same — but only because A had 2 entries |
| 3 | A → B | Summary: `{alice: 15, bob: 3}` | 2 entries | 2 of 3 entries (B already knows carol) |
| 4 | B → A | Delta: `{carol: 7}` | **1 entry** | **vs 3 entries in full state** |

The savings grow with state size. For a GCounter with 1000 replicas where only 1
has changed, the delta is 1 entry instead of 1000.

## Generic Protocol

```mermaid
sequenceDiagram
    participant Local
    participant Remote

    Note over Local,Remote: Any CRDT implementing DeltaSync

    Local->>Remote: summary = Local.summary()
    Note right of Remote: Compact metadata describing<br/>what Local already knows
    Remote->>Remote: delta = Remote.delta_from_summary(&summary)
    Note right of Remote: Compute minimal state fragment<br/>containing only what Local is missing
    Remote->>Local: delta
    Local->>Local: Local.merge_delta(&delta)
    Note left of Local: Local now knows everything<br/>Remote knew at summary time
```

## Key Invariant

For any two states A and B:

```
A ⊔ δ(B, σ(A)) = A ⊔ B
```

Delta sync produces the **same result** as full-state merge — it's just more efficient
on the wire because the delta `δ(B, σ(A))` is typically much smaller than `B`.
