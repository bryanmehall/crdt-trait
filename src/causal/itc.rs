use crate::{Apply, Crdt, Replica};
use std::borrow::Cow;
use std::cmp;

// --- ID TREE (Identity) ---

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdTree {
    Leaf {
        i: bool,
    },
    Node {
        left: Box<IdTree>,
        right: Box<IdTree>,
    },
}

impl IdTree {
    pub fn zero() -> Self {
        IdTree::Leaf { i: false }
    }
    pub fn one() -> Self {
        IdTree::Leaf { i: true }
    }
    pub fn node(left: Box<IdTree>, right: Box<IdTree>) -> Self {
        IdTree::Node { left, right }
    }
}

// --- EVENT TREE (State) ---

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventTree {
    Leaf {
        n: u32,
    },
    Node {
        n: u32,
        left: Box<EventTree>,
        right: Box<EventTree>,
    },
}

impl EventTree {
    pub fn zero() -> Self {
        EventTree::Leaf { n: 0 }
    }
    pub fn leaf(n: u32) -> Self {
        EventTree::Leaf { n }
    }
    pub fn node(n: u32, left: Box<EventTree>, right: Box<EventTree>) -> Self {
        EventTree::Node { n, left, right }
    }
}

// --- COST (Helper for balancing) ---

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Cost(i32);

impl Cost {
    fn zero() -> Self {
        Cost(0)
    }
    fn shift(self) -> Self {
        Cost(self.0 + 1)
    }
}

impl std::ops::Add<i32> for Cost {
    type Output = Cost;
    fn add(self, rhs: i32) -> Cost {
        Cost(self.0 + rhs)
    }
}

// --- WRAPPERS ---

/// The Identity resource for an Interval Tree Clock.
///
/// This represents a unique portion of the identity space [0, 1].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItcId(pub IdTree);

/// The Replica manager for ITC.
///
/// Handles forking and joining identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItcReplica {
    pub tree: IdTree,
}

impl Default for ItcReplica {
    fn default() -> Self {
        ItcReplica {
            tree: IdTree::one(),
        }
    }
}

impl ItcReplica {
    /// Creates a new generic "Seed" replica (owns the entire ID space).
    pub fn new() -> Self {
        Self::default()
    }
}

impl Replica for ItcReplica {
    type Id = ItcId;

    fn id(&self) -> Self::Id {
        ItcId(self.tree.clone())
    }

    fn fork(&mut self) -> Self {
        let (my_new_tree, other_tree) = self.tree.split();
        self.tree = my_new_tree;
        ItcReplica { tree: other_tree }
    }

    fn join(&mut self, other: Self) {
        self.tree = self.tree.sum(&other.tree);
    }
}

/// The Event Clock for ITC.
///
/// Tracks causality using an Interval Tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItcClock {
    pub tree: EventTree,
}

impl Default for ItcClock {
    fn default() -> Self {
        ItcClock {
            tree: EventTree::zero(),
        }
    }
}

impl Crdt for ItcClock {
    type Value = EventTree; // The raw tree is the value

    fn merge(&mut self, other: &Self) {
        self.tree = self.tree.join(&other.tree);
    }

    fn value(&self) -> Self::Value {
        self.tree.clone()
    }
}

impl Apply for ItcClock {
    type Op = (); // A "tick" is just an event, no payload
    type Context = ItcId; // Requires the Identity to fill/grow

    fn apply(&mut self, _op: Self::Op, ctx: Self::Context) {
        let id = ctx.0;

        // 1. Fill
        let filled_cow = self.tree.fill(&id);

        // 2. Check if fill changed anything
        if filled_cow.as_ref() != &self.tree {
            self.tree = filled_cow.into_owned();
        } else {
            // 3. Grow if fill didn't change anything
            let (grown_tree, _) = self.tree.grow(&id);
            self.tree = grown_tree;
        }
    }
}

// --- IMPLEMENTATION LOGIC ---

trait Min<T> {
    fn min(&self) -> T;
}
trait Max<T> {
    fn max(&self) -> T;
}
trait Normalisable {
    fn norm(self) -> Self;
}

impl Min<u32> for EventTree {
    fn min(&self) -> u32 {
        match *self {
            EventTree::Leaf { n } => n,
            EventTree::Node {
                n,
                ref left,
                ref right,
            } => n + cmp::min(left.min(), right.min()),
        }
    }
}

impl Max<u32> for EventTree {
    fn max(&self) -> u32 {
        match *self {
            EventTree::Leaf { n } => n,
            EventTree::Node {
                n,
                ref left,
                ref right,
            } => n + cmp::max(left.max(), right.max()),
        }
    }
}

impl Normalisable for IdTree {
    fn norm(self) -> IdTree {
        match self {
            IdTree::Leaf { .. } => self,
            IdTree::Node { left, right } => {
                let norm_left = left.norm();
                let norm_right = right.norm();
                match (&norm_left, &norm_right) {
                    (IdTree::Leaf { i: i1 }, IdTree::Leaf { i: i2 }) if i1 == i2 => norm_left,
                    _ => IdTree::node(Box::new(norm_left), Box::new(norm_right)),
                }
            }
        }
    }
}

impl Normalisable for EventTree {
    fn norm(self) -> EventTree {
        match self {
            EventTree::Leaf { .. } => self,
            EventTree::Node { n, left, right } => {
                let norm_left = left.norm();
                let norm_right = right.norm();

                if let (EventTree::Leaf { n: m1 }, EventTree::Leaf { n: m2 }) =
                    (&norm_left, &norm_right)
                {
                    if m1 == m2 {
                        return EventTree::leaf(n + m1);
                    }
                }

                let min_left = norm_left.n(); // n() extracts n from leaf or node
                let min_right = norm_right.n();
                let m = cmp::min(min_left, min_right);

                EventTree::node(
                    n + m,
                    Box::new(norm_left.sink(m)),
                    Box::new(norm_right.sink(m)),
                )
            }
        }
    }
}

impl EventTree {
    fn n(&self) -> u32 {
        match self {
            EventTree::Leaf { n } => *n,
            EventTree::Node { n, .. } => *n,
        }
    }

    fn lift(self, m: u32) -> EventTree {
        match self {
            EventTree::Leaf { n } => EventTree::leaf(n + m),
            EventTree::Node { n, left, right } => EventTree::node(n + m, left, right),
        }
    }

    fn sink(self, m: u32) -> EventTree {
        match self {
            EventTree::Leaf { n } => EventTree::leaf(n - m),
            EventTree::Node { n, left, right } => EventTree::node(n - m, left, right),
        }
    }

    fn join(&self, other: &EventTree) -> EventTree {
        match (self, other) {
            (EventTree::Leaf { n: n1 }, EventTree::Leaf { n: n2 }) => {
                EventTree::leaf(cmp::max(*n1, *n2))
            }
            (EventTree::Leaf { n: n1 }, EventTree::Node { .. }) => {
                let new_left = EventTree::node(
                    *n1,
                    Box::new(EventTree::zero()),
                    Box::new(EventTree::zero()),
                );
                new_left.join(other)
            }
            (EventTree::Node { .. }, EventTree::Leaf { n: n2 }) => {
                let new_right = EventTree::node(
                    *n2,
                    Box::new(EventTree::zero()),
                    Box::new(EventTree::zero()),
                );
                self.join(&new_right)
            }
            (
                EventTree::Node {
                    n: n1,
                    left: left1,
                    right: right1,
                },
                EventTree::Node {
                    n: n2,
                    left: left2,
                    right: right2,
                },
            ) => {
                if n1 > n2 {
                    other.join(self)
                } else {
                    let diff = n2 - n1;
                    let new_left = left1.join(&left2.clone().lift(diff));
                    let new_right = right1.join(&right2.clone().lift(diff));
                    EventTree::node(*n1, Box::new(new_left), Box::new(new_right)).norm()
                }
            }
        }
    }

    // Fill implementation adapted from source
    fn fill<'a>(&'a self, id: &IdTree) -> Cow<'a, EventTree> {
        if *id == IdTree::zero() {
            Cow::Borrowed(self)
        } else if *id == IdTree::one() {
            Cow::Owned(EventTree::leaf(self.max()))
        } else if let EventTree::Leaf { .. } = self {
            Cow::Borrowed(self)
        } else {
            if let IdTree::Node {
                left: i_left,
                right: i_right,
            } = id
            {
                if let EventTree::Node {
                    n,
                    left: e_left,
                    right: e_right,
                } = self
                {
                    if **i_left == IdTree::one() {
                        let eprime_right = e_right.fill(i_right).into_owned();
                        let new_left = EventTree::leaf(cmp::max(e_left.max(), eprime_right.min()));
                        Cow::Owned(
                            EventTree::node(*n, Box::new(new_left), Box::new(eprime_right)).norm(),
                        )
                    } else if **i_right == IdTree::one() {
                        let eprime_left = e_left.fill(i_left).into_owned();
                        let new_right = EventTree::leaf(cmp::max(e_right.max(), eprime_left.min()));
                        Cow::Owned(
                            EventTree::node(*n, Box::new(eprime_left), Box::new(new_right)).norm(),
                        )
                    } else {
                        let new_left = e_left.fill(i_left).into_owned();
                        let new_right = e_right.fill(i_right).into_owned();
                        Cow::Owned(
                            EventTree::node(*n, Box::new(new_left), Box::new(new_right)).norm(),
                        )
                    }
                } else {
                    unreachable!()
                }
            } else {
                unreachable!()
            }
        }
    }

    fn grow(&self, id: &IdTree) -> (EventTree, Cost) {
        match self {
            EventTree::Leaf { n } => {
                if *id == IdTree::one() {
                    (EventTree::leaf(n + 1), Cost::zero())
                } else {
                    let new_e = EventTree::node(
                        *n,
                        Box::new(EventTree::zero()),
                        Box::new(EventTree::zero()),
                    );
                    let (eprime, c) = new_e.grow(id);
                    (eprime, c.shift())
                }
            }
            EventTree::Node {
                n,
                left: e_left,
                right: e_right,
            } => {
                if let IdTree::Node {
                    left: i_left,
                    right: i_right,
                } = id
                {
                    if **i_left == IdTree::zero() {
                        let (eprime_right, c_right) = e_right.grow(i_right);
                        (
                            EventTree::node(*n, e_left.clone(), Box::new(eprime_right)),
                            c_right + 1,
                        )
                    } else if **i_right == IdTree::zero() {
                        let (eprime_left, c_left) = e_left.grow(i_left);
                        (
                            EventTree::node(*n, Box::new(eprime_left), e_right.clone()),
                            c_left + 1,
                        )
                    } else {
                        let (eprime_right, c_right) = e_right.grow(i_right);
                        let (eprime_left, c_left) = e_left.grow(i_left);
                        if c_left < c_right {
                            (
                                EventTree::node(*n, Box::new(eprime_left), e_right.clone()),
                                c_left + 1,
                            )
                        } else {
                            (
                                EventTree::node(*n, e_left.clone(), Box::new(eprime_right)),
                                c_right + 1,
                            )
                        }
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }
}

impl IdTree {
    fn split(&self) -> (Self, Self) {
        match self {
            IdTree::Leaf { i } => {
                if !i {
                    (IdTree::zero(), IdTree::zero())
                } else {
                    // Split 1 into (1,0) and (0,1)
                    let left = IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::zero()));
                    let right = IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::one()));
                    (left, right)
                }
            }
            IdTree::Node { left, right } => {
                if **left == IdTree::zero() {
                    let (i1, i2) = right.split();
                    let new_left = IdTree::node(Box::new(IdTree::zero()), Box::new(i1));
                    let new_right = IdTree::node(Box::new(IdTree::zero()), Box::new(i2));
                    (new_left, new_right)
                } else if **right == IdTree::zero() {
                    let (i1, i2) = left.split();
                    let new_left = IdTree::node(Box::new(i1), Box::new(IdTree::zero()));
                    let new_right = IdTree::node(Box::new(i2), Box::new(IdTree::zero()));
                    (new_left, new_right)
                } else {
                    let new_left = IdTree::node(left.clone(), Box::new(IdTree::zero()));
                    let new_right = IdTree::node(Box::new(IdTree::zero()), right.clone());
                    (new_left, new_right)
                }
            }
        }
    }

    fn sum(&self, other: &Self) -> Self {
        if *self == IdTree::zero() {
            return other.clone();
        }
        if *other == IdTree::zero() {
            return self.clone();
        }

        match (self, other) {
            (
                IdTree::Node {
                    left: l1,
                    right: r1,
                },
                IdTree::Node {
                    left: l2,
                    right: r2,
                },
            ) => {
                let new_left = l1.sum(l2);
                let new_right = r1.sum(r2);
                IdTree::node(Box::new(new_left), Box::new(new_right)).norm()
            }
            _ => unreachable!(),
        }
    }
}
