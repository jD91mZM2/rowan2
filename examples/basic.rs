extern crate rowan2;

use rowan2::{Node, TreeBuilder};

#[derive(Clone, Copy, Debug)]
enum SyntaxKind {
    Group,
    Number,
    Operation
}

fn recurse<R: rowan2::TreeRoot<SyntaxKind>>(indent: usize, node: Node<SyntaxKind, R>) {
    println!("{:indent$}{:?}", "", node, indent = indent);

    for child in node.children() {
        recurse(indent+2, child);
    }
}
fn main() {
    let mut builder = TreeBuilder::new();

    builder.start_internal(SyntaxKind::Group);
    builder.start_internal(SyntaxKind::Group);
    builder.leaf(SyntaxKind::Number, "1".into());
    builder.leaf(SyntaxKind::Operation, "+".into());
    builder.start_internal(SyntaxKind::Group);
    builder.leaf(SyntaxKind::Number, "2".into());
    builder.leaf(SyntaxKind::Operation, "*".into());
    builder.leaf(SyntaxKind::Number, "3".into());
    builder.finish_internal();
    builder.finish_internal();
    builder.leaf(SyntaxKind::Operation, "-".into());
    builder.leaf(SyntaxKind::Number, "4".into());
    builder.finish_internal();

    let node = builder.finish();

    recurse(0, node.borrowed());

    let child = node.first_child().unwrap();
    for (nested, event) in child.walk() {
        println!("{:indent$}{:?}", "", event, indent = nested * 2);
    }

    println!("{}", node);
}
