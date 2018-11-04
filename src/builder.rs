use crate::node::{MutableRoot, Node, RootData, OwnedRoot};
use smol_str::SmolStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct NodeId(pub(crate) usize);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Content {
    Branch(Option<NodeId>),
    Leaf(SmolStr)
}
impl Content {
    pub(crate) fn expect_branch(&mut self) -> &mut Option<NodeId> {
        match self {
            Content::Branch(node) => node,
            Content::Leaf(_) => panic!("expected branch, found leaf node")
        }
    }
}

#[derive(Debug)]
pub(crate) struct NodeRepr<T: Copy> {
    pub(crate) kind: T,

    pub(crate) parent: Option<NodeId>,
    pub(crate) prev_sibling: Option<NodeId>,
    pub(crate) next_sibling: Option<NodeId>,
    pub(crate) content: Content
}

/// See the function `checkpoint` in `TreeBuilder`
#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    cursor: u32,
    child: Option<NodeId>
}

/// A builder for trees, supplying functions for starting/ending branches
#[derive(Debug)]
pub struct TreeBuilder<T: Copy> {
    arena: Vec<Option<NodeRepr<T>>>,
    parent: Option<NodeId>,
    child: Option<NodeId>,

    ranges: Vec<(u32, Option<u32>)>,
    cursor: u32
}
impl<T: Copy> Default for TreeBuilder<T> {
    fn default() -> Self {
        Self {
            arena: Vec::new(),
            parent: None,
            child: None,

            ranges: Vec::new(),
            cursor: 0
        }
    }
}
impl<T: Copy> TreeBuilder<T> {
    /// Create a new instance
    pub fn new() -> Self {
        Self::default()
    }
    fn get(&mut self, id: Option<NodeId>) -> Option<&mut NodeRepr<T>> {
        id.map(move |id| self.arena[id.0].as_mut().unwrap())
    }
    fn parent(&mut self) -> Option<&mut NodeRepr<T>> {
        let id = self.parent; self.get(id)
    }
    fn child(&mut self) -> Option<&mut NodeRepr<T>> {
        let id = self.child; self.get(id)
    }
    fn insert(&mut self, node: NodeRepr<T>) -> NodeId {
        let id = NodeId(self.arena.len());
        self.arena.push(Some(node));
        id
    }
    fn insert_and_update(&mut self, kind: T, content: Content) -> NodeId {
        let node = NodeRepr {
            kind,

            parent: self.parent,
            prev_sibling: self.child,
            next_sibling: None,
            content
        };
        let id = self.insert(node);

        if let Some(node) = self.parent() {
            let child = node.content.expect_branch();
            *child = child.or(Some(id));
        }
        if let Some(node) = self.child() {
            node.next_sibling = Some(id);
        }

        id
    }
    /// Start a new branch and switch to it
    pub fn start_internal(&mut self, kind: T) {
        self.ranges.push((self.cursor, None));

        let id = self.insert_and_update(kind, Content::Branch(None));
        self.parent = Some(id);
        self.child = None;
    }
    /// End a previously started branch
    pub fn finish_internal(&mut self) {
        if let Some(parent) = self.parent {
            let end = self.child.map(|id| self.ranges[id.0].1.unwrap())
                .unwrap_or(self.ranges[parent.0].0);
            // Update the end position of the range
            self.ranges[parent.0].1 = Some(end);
        }

        self.child = self.parent;
        self.parent = self.parent().and_then(|node| node.parent);
    }
    /// Put a leaf in the current branch
    pub fn leaf(&mut self, kind: T, text: SmolStr) {
        self.ranges.push((self.cursor, Some(self.cursor + text.len() as u32)));
        self.cursor += text.len() as u32;

        let id = self.insert_and_update(kind, Content::Leaf(text));
        self.child = Some(id);
    }
    /// Save a "checkpoint", allowing you to wrap everything since here in
    /// another node, using `start_internal_at`
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            cursor: self.cursor,
            child: self.child
        }
    }
    /// This wraps everything after a checkpoint in a node with the specified
    /// kind. This is invaluable for parsing for example `1 + 2`, where you
    /// don't before hand know if `1` should be wrapped like `Operation(Number,
    /// Number)` or just be a `Number`
    pub fn start_internal_at(&mut self, checkpoint: Checkpoint, kind: T) {
        self.ranges.push((checkpoint.cursor, None));

        let previous = checkpoint.child;
        match previous {
            None => {
                // No children at the time of the checkpoint, update parent
                let mut old_id = self.parent().and_then(|node| *node.content.expect_branch());
                let node = NodeRepr {
                    kind,

                    parent: self.parent,
                    prev_sibling: None,
                    next_sibling: None,
                    content: Content::Branch(old_id)
                };
                let id = self.insert(node);
                while let Some(old) = self.get(old_id) {
                    old.parent = Some(id);
                    old_id = old.next_sibling;
                }
                if let Some(parent) = self.parent() {
                    parent.content = Content::Branch(Some(id));
                }
                self.parent = Some(id);
            },
            Some(_) => {
                // Update previous entry, which is checkpoint.child
                let mut old_id = self.get(previous).and_then(|node| node.next_sibling);
                let node = NodeRepr {
                    kind,

                    parent: self.get(previous).unwrap().parent,
                    prev_sibling: previous,
                    next_sibling: None,
                    content: Content::Branch(old_id)
                };
                let id = self.insert(node);
                if let Some(old) = self.get(old_id) {
                    old.prev_sibling = None;
                }
                while let Some(old) = self.get(old_id) {
                    old.parent = Some(id);
                    old_id = old.next_sibling;
                }
                self.get(previous).unwrap().next_sibling = Some(id);
                self.parent = Some(id);
            }
        }
    }
    /// Build the tree, returning an immutable owned tree
    pub fn finish(mut self) -> Node<T, OwnedRoot<T>> {
        assert!(self.child.is_some(), "finish called on empty builder");
        assert!(self.child().unwrap().prev_sibling.is_none(), "can't finish on more than one node");

        Node::new_root(
            RootData {
                arena: self.arena,
                ranges: self.ranges
            },
            self.child.unwrap()
        )
    }
    /// Build the tree, returning an mutable owned tree with all ranges
    /// discarded
    pub fn finish_mut(mut self) -> Node<T, MutableRoot<T>> {
        assert!(self.child.is_some(), "finish called on empty builder");
        assert!(self.child().unwrap().prev_sibling.is_none(), "can't finish on more than one node");

        Node::new_root_mut(
            RootData {
                arena: self.arena,
                ranges: Vec::new()
            },
            self.child.unwrap()
        )
    }
}
