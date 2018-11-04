extern crate rowan2;

use rowan2::{TreeBuilder, WalkEvent};

#[derive(Clone, Copy, Debug)]
enum SyntaxKind {
    Group,
    Number,
    Operation
}

fn main() {
    let mut builder = TreeBuilder::new();

    builder.start_internal(SyntaxKind::Group);
    builder.start_internal(SyntaxKind::Group);
    builder.leaf(SyntaxKind::Number, "1".into());
    let checkpoint = builder.checkpoint();
    builder.leaf(SyntaxKind::Operation, "+".into());

    builder.leaf(SyntaxKind::Number, "2".into());
    builder.leaf(SyntaxKind::Operation, "*".into());
    builder.start_internal_at(checkpoint, SyntaxKind::Group);
    builder.finish_internal();
    builder.leaf(SyntaxKind::Number, "3".into());

    builder.finish_internal();
    builder.leaf(SyntaxKind::Operation, "-".into());
    builder.leaf(SyntaxKind::Number, "4".into());
    builder.finish_internal();

    let node = builder.finish();

    for (nested, event) in node.borrowed().walk() {
        if let WalkEvent::Enter(node) = event {
            println!("{:indent$}{:?} {:?}", "", node, node.leaf_text_cow(), indent = nested * 2);
        }
    }
}
