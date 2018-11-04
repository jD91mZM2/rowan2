use crate::{
    builder::{Content, NodeId, NodeRepr},
    lock::{Lock, RefCount}
};

use smol_str::SmolStr;
use std::{
    borrow::Cow,
    fmt::{self, Debug, Display},
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::DerefMut
};
use text_unit::{TextRange, TextUnit};

/// The root data of the tree, such as the node arena
#[derive(Debug)]
pub struct RootData<T: Copy> {
    pub(crate) arena: Vec<Option<NodeRepr<T>>>,
    pub(crate) ranges: Vec<(u32, Option<u32>)>
}

/// An internal trait for allowing multiple ways to access the tree root.
/// Don't implement this yourself, instead use for example `OwnedRoot` or
/// `RefRoot`.
pub trait TreeRoot<T: Copy>: Clone {
    type Borrowed: TreeRoot<T>;
    fn with_data<F, V>(&self, f: F) -> V
        where F: FnOnce(&RootData<T>) -> V;
    fn borrow_data(&self) -> Option<&RootData<T>>;
    fn borrowed(&self) -> RefRoot<T, Self::Borrowed>;
}

/// A tree root that allows you to mutate inner data by using interior
/// mutability. Very similar to `OwnedRoot`.
#[derive(Clone, Debug)]
pub struct MutableRoot<T: Copy>(RefCount<Lock<RootData<T>>>);
impl<T: Copy> TreeRoot<T> for MutableRoot<T> {
    type Borrowed = Self;
    fn with_data<F, V>(&self, f: F) -> V
        where F: FnOnce(&RootData<T>) -> V
    {
        f(&self.0.read())
    }
    fn borrow_data(&self) -> Option<&RootData<T>> {
        None
    }
    fn borrowed(&self) -> RefRoot<T, Self::Borrowed> {
        RefRoot {
            inner: self,
            _marker: PhantomData::default()
        }
    }
}
/// An immutable tree root that reference counts the inner data, allowing you
/// to own the tree and not get lifetime issues. For processing nodes you
/// should always use RefRoot for performance.
#[derive(Clone, Debug)]
pub struct OwnedRoot<T: Copy>(RefCount<RootData<T>>);
impl<T: Copy> TreeRoot<T> for OwnedRoot<T> {
    type Borrowed = Self;
    fn with_data<F, V>(&self, f: F) -> V
        where F: FnOnce(&RootData<T>) -> V
    {
        f(&self.0)
    }
    fn borrow_data(&self) -> Option<&RootData<T>> {
        Some(&self.0)
    }
    fn borrowed(&self) -> RefRoot<T, Self::Borrowed> {
        RefRoot {
            inner: self,
            _marker: PhantomData::default()
        }
    }
}
/// A tree root that forwards usages to another tree root using a reference.
/// Good for processing nodes because it avoids reference counters. Since it
/// doesn't own the data, this might get you in trouble with lifetimes if you
/// try to store nodes of this type.
#[derive(Clone, Debug)]
pub struct RefRoot<'a, T: Copy + 'a, R: TreeRoot<T> + 'a> {
    inner: &'a R,
    _marker: PhantomData<T>
}
impl<'a, T: Copy, R: TreeRoot<T>> Copy for RefRoot<'a, T, R> {}
impl<'a, T: Copy, R: TreeRoot<T>> TreeRoot<T> for RefRoot<'a, T, R> {
    type Borrowed = R;
    fn with_data<F, V>(&self, f: F) -> V
        where F: FnOnce(&RootData<T>) -> V
    {
        self.inner.with_data(f)
    }
    fn borrow_data(&self) -> Option<&RootData<T>> {
        self.inner.borrow_data()
    }
    fn borrowed(&self) -> RefRoot<T, Self::Borrowed> {
        *self
    }
}

/// The node type
#[derive(Clone, Eq)]
pub struct Node<T: Copy, R: TreeRoot<T>> {
    root: R,
    node: NodeId,
    _marker: PhantomData<T>
}
impl<T: Copy> Node<T, OwnedRoot<T>> {
    pub(crate) fn new_root(data: RootData<T>, node: NodeId) -> Self {
        Node {
            root: OwnedRoot(RefCount::new(data)),
            node,
            _marker: PhantomData::default()
        }
    }
}
impl<'a, T: Copy> Node<T, RefRoot<'a, T, OwnedRoot<T>>> {
    /// Switch this borrowed node to an owned one. This performes a clone on
    /// the reference counter.
    pub fn owned(&self) -> Node<T, OwnedRoot<T>> {
        Node {
            root: self.root.inner.clone(),
            node: self.node,
            _marker: PhantomData::default()
        }
    }
    /// Convert this borrowed node into its inner leaf text with the same
    /// lifetime.
    pub fn leaf_text(self) -> Option<&'a SmolStr> {
        let data = &self.root.inner.0;
        let repr = data.arena[self.node.0].as_ref().unwrap();
        match repr.content {
            Content::Branch(_) => None,
            Content::Leaf(ref s) => Some(s)
        }
    }
}
impl<T: Copy> Node<T, MutableRoot<T>> {
    pub(crate) fn new_root_mut(data: RootData<T>, node: NodeId) -> Self {
        Node {
            root: MutableRoot(RefCount::new(Lock::new(data))),
            node,
            _marker: PhantomData::default()
        }
    }
    fn data_mut<'a>(&'a self) -> impl DerefMut<Target = RootData<T>> + 'a {
        self.root.0.write()
    }
    /// Remove this node from the tree. This frees all children.
    pub fn remove(self) {
        let mut data = self.data_mut();
        let repr = data.arena[self.node.0].take().unwrap();

        // Free all children
        let mut next = match repr.content {
            Content::Branch(child) => child,
            Content::Leaf(_) => None
        };
        while let Some(current) = next {
            next = data.arena[current.0].take().unwrap().next_sibling;
        }

        if let Some(prev_sibling) = repr.prev_sibling {
            // Remove the node by linking the previous node directly to the next
            data.arena[prev_sibling.0].as_mut().unwrap().next_sibling = repr.next_sibling;
        } else if let Some(parent) = repr.parent {
            // Remove the node by linking the parent directly to the next
            *data.arena[parent.0].as_mut().unwrap().content.expect_branch() = repr.next_sibling;
        }
    }
    /// Insert a new node right before this node
    pub fn insert_before(&self, kind: T, content: Option<SmolStr>) -> Self {
        let mut data = self.data_mut();
        let node = {
            let repr = data.arena[self.node.0].as_ref().unwrap();
            NodeRepr {
                kind,

                parent: repr.parent,
                prev_sibling: repr.prev_sibling,
                next_sibling: Some(self.node),
                content: match content {
                    None => Content::Branch(None),
                    Some(text) => Content::Leaf(text)
                }
            }
        };
        let id = NodeId(data.arena.len());
        data.arena.push(Some(node));

        {
            if let Some(prev_sibling) = data.arena[self.node.0].as_ref().unwrap().prev_sibling {
                data.arena[prev_sibling.0].as_mut().unwrap().next_sibling = Some(id);
            }
            if let Some(parent) = data.arena[self.node.0].as_ref().unwrap().parent {
                let parent = data.arena[parent.0].as_mut().unwrap();
                if parent.content == Content::Branch(Some(self.node)) {
                    parent.content = Content::Branch(Some(id));
                }
            }
        }
        data.arena[self.node.0].as_mut().unwrap().prev_sibling = Some(id);

        self.with_node(id)
    }
    /// Insert a new node directly after this node
    pub fn insert_after(&self, kind: T, content: Option<SmolStr>) -> Self {
        let mut data = self.data_mut();
        let node = {
            let repr = data.arena[self.node.0].as_ref().unwrap();
            NodeRepr {
                kind,

                parent: repr.parent,
                prev_sibling: Some(self.node),
                next_sibling: repr.next_sibling,
                content: match content {
                    None => Content::Branch(None),
                    Some(text) => Content::Leaf(text)
                }
            }
        };
        let id = NodeId(data.arena.len());
        data.arena.push(Some(node));

        {
            if let Some(next_sibling) = data.arena[self.node.0].as_ref().unwrap().next_sibling {
                data.arena[next_sibling.0].as_mut().unwrap().prev_sibling = Some(id);
            }
        }
        data.arena[self.node.0].as_mut().unwrap().next_sibling = Some(id);

        self.with_node(id)
    }
}
impl<T: Copy, R: TreeRoot<T>> Node<T, R> {
    /// Borrow this node, getting a cheap node type that implements Copy. See
    /// RefRoot for details.
    pub fn borrowed(&self) -> Node<T, RefRoot<T, R::Borrowed>> {
        Node {
            root: self.root.borrowed(),
            node: self.node,
            _marker: PhantomData::default()
        }
    }
    fn repr<F, V>(&self, f: F) -> V
        where F: FnOnce(&NodeRepr<T>) -> V
    {
        self.root.with_data(move |data| {
            f(&data.arena[self.node.0].as_ref().unwrap())
        })
    }
    fn with_node(&self, node: NodeId) -> Self {
        Node {
            root: self.root.clone(),
            node,
            _marker: PhantomData::default()
        }
    }
    /// Get the parent node
    pub fn parent(&self) -> Option<Self> {
        self.repr(|repr| repr.parent).map(|node| self.with_node(node))
    }
    /// Get the next sibling
    pub fn next_sibling(&self) -> Option<Self> {
        self.repr(|repr| repr.next_sibling).map(|node| self.with_node(node))
    }
    /// Get the previous sibling
    pub fn prev_sibling(&self) -> Option<Self> {
        self.repr(|repr| repr.prev_sibling).map(|node| self.with_node(node))
    }
    /// Get the first child
    pub fn first_child(&self) -> Option<Self> {
        self.repr(|repr| match repr.content {
            Content::Branch(child) => child.map(|node| self.with_node(node)),
            Content::Leaf(_) => None
        })
    }
    /// Get an iterator over all children
    pub fn children(&self) -> NodeIter<T, R> {
        NodeIter {
            next: self.first_child()
        }
    }
    /// Get the leaf text. If the tree root is mutable this will clone the text.
    pub fn leaf_text_cow(&self) -> Option<Cow<SmolStr>> {
        if let Some(data) = self.root.borrow_data() {
            let repr = data.arena[self.node.0].as_ref().unwrap();
            match repr.content {
                Content::Branch(_) => None,
                Content::Leaf(ref s) => Some(Cow::Borrowed(s))
            }
        } else {
            self.repr(|repr| match repr.content {
                Content::Branch(_) => None,
                Content::Leaf(ref s) => Some(Cow::Owned(s.clone()))
            })
        }
    }
    /// Try getting the range. This will always succeed on immutable tree
    /// roots, but always fail on mutable onces as they don't store range data.
    pub fn try_range(&self) -> Option<TextRange> {
        self.root.with_data(|data| {
            if data.ranges.is_empty() {
                return None;
            }
            let range = data.ranges[self.node.0];
            Some(TextRange::from_to(TextUnit::from(range.0), TextUnit::from(range.1.unwrap())))
        })
    }
    /// Get the text range
    ///
    /// # Panics
    /// This function panics if the tree root is mutable, because those don't store range data
    pub fn range(&self) -> TextRange {
        self.try_range().expect("node is mutable and doesn't have a range")
    }
    /// Get the node kind
    pub fn kind(&self) -> T {
        self.repr(|repr| repr.kind)
    }
    /// Return an iterator that traverses this tree
    pub fn walk(&self) -> NodeWalker<T, R> {
        NodeWalker {
            next: Some(WalkEvent::Enter(self.clone())),
            nested: 0
        }
    }
}
impl<T: Copy + Debug, R: TreeRoot<T>> Debug for Node<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(range) = self.try_range() {
            write!(f, "{:?}@{:?}", self.kind(), range)
        } else {
            write!(f, "{:?}@MUT", self.kind())
        }
    }
}
impl<T: Copy, R: TreeRoot<T>> Display for Node<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (_, event) in self.borrowed().walk() {
            if let WalkEvent::Enter(node) = event {
                if let Some(text) = node.leaf_text_cow() {
                    write!(f, "{}", text)?;
                }
            }
        }
        Ok(())
    }
}
impl<T: Copy, R: TreeRoot<T>> PartialEq for Node<T, R> {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}
impl<T: Copy, R: TreeRoot<T>> Hash for Node<T, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node.hash(state);
    }
}

pub struct NodeIter<T: Copy, R: TreeRoot<T>> {
    next: Option<Node<T, R>>
}
impl<T: Copy, R: TreeRoot<T>> Iterator for NodeIter<T, R> {
    type Item = Node<T, R>;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next.take();
        if let Some(ref node) = node {
            self.next = node.next_sibling();
        }
        node
    }
}

pub enum WalkEvent<T: Copy, R: TreeRoot<T>> {
    Enter(Node<T, R>),
    Leave(Node<T, R>)
}
impl<T: Copy + Debug, R: TreeRoot<T>> Debug for WalkEvent<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WalkEvent::Enter(node) => write!(f, "> Enter({:?})", node),
            WalkEvent::Leave(node) => write!(f, "< Leave({:?})", node),
        }
    }
}

pub struct NodeWalker<T: Copy, R: TreeRoot<T>> {
    /// the next event
    next: Option<WalkEvent<T, R>>,
    /// how many levels deep we are. this is used to stop once we reach the
    /// same parent we started at.
    nested: usize
}
impl<T: Copy, R: TreeRoot<T>> Iterator for NodeWalker<T, R> {
    type Item = (usize, WalkEvent<T, R>);
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next.take();
        let (nested, new) = match next {
            None => (0, None),
            Some(WalkEvent::Enter(ref node)) => {
                let old_nested = self.nested;
                self.nested += 1;
                (old_nested, Some(match node.first_child() {
                    Some(child) => WalkEvent::Enter(child),
                    None => WalkEvent::Leave(node.clone())
                }))
            },
            Some(WalkEvent::Leave(ref node)) => {
                self.nested -= 1;
                (self.nested, if self.nested == 0 {
                    None
                } else {
                    match node.next_sibling() {
                        Some(next) => Some(WalkEvent::Enter(next)),
                        None => node.parent().map(WalkEvent::Leave)
                    }
                })
            }
        };
        self.next = new;
        next.map(|next| (nested, next))
    }
}
