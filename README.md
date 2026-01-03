# CRDT Trait

[![CI](https://github.com/bryanmehall/crdt-trait/actions/workflows/ci.yml/badge.svg)](https://github.com/bryanmehall/crdt-trait/actions/workflows/ci.yml)

A Rust library for creating creating robust distributed systems using custom Conflict-Free Replicated Datatypes.
This crate provides a well tested `CRDT` trait that lets you create custom, serializable CRDTs for your domain specific applications so that you can confidently create eventually consistent distributed systems.
It also includes off the shelf implementations of common CRDTs with examples that can be used directly or composed into larger CRDT data strucutres.
## Example
``` rust
#[derive(CRDT, Serialize, Deserialize)]
pub struct CustomCRDT {
    grow_only_field: 0
}

let node_1_state = CustomCRDT::init();
let node_2_state = node_1_state.fork();

node_1_state.update(|&mut old_state| old_state.grow_only_field += 1 );
```


## Eventual Consistency
Eventually consistent data structures allow the state of nodes within a distributed system to diverge when disconnected and then re-combine to an agreed upon state without central coordination.
More formally, eventually consistent data structure must obey the following properties:
### Idempotence
_An update applied multiple times will have the same effect as if it is only applied once_

For a state update $A$ and a merge operator $\sqcup$, $A \sqcup A = A$

### Commutativity
_The order of applying updates does not matter_

For state updates $A$ and $B$ and a merge operator $\sqcup$, $A \sqcup B = B \sqcup A$

### Associativity
_Updates from several nodes can be grouped arbitrarily_

For state updates $A$, $B$, and $C$, and a merge operator $\sqcup$, $(A \sqcup B) \sqcup C = A \sqcup (B \sqcup C)$



# Features
* **`serde`**: Make your CRDTs serializable
* **`proptest`**: Automatically generate tests to ensure your CRDTs will always converge to the same state
