extern crate rowan2;

use rowan2::TreeBuilder;

#[derive(Clone, Copy, Debug)]
enum SyntaxKind {
    Group,
    Number,
    Operation,
    Paren
}

fn main() {
    let mut builder = TreeBuilder::new();

    builder.start_internal(SyntaxKind::Group);

    builder.start_internal(SyntaxKind::Group);
    builder.leaf(SyntaxKind::Paren, "(".into());
    builder.leaf(SyntaxKind::Number, "1".into());
    builder.leaf(SyntaxKind::Operation, "+".into());
    builder.start_internal(SyntaxKind::Group);
    builder.leaf(SyntaxKind::Paren, "(".into());
    builder.leaf(SyntaxKind::Number, "2".into());
    builder.leaf(SyntaxKind::Operation, "*".into());
    builder.leaf(SyntaxKind::Number, "3".into());
    builder.leaf(SyntaxKind::Paren, ")".into());
    builder.finish_internal();
    builder.leaf(SyntaxKind::Paren, ")".into());
    builder.finish_internal();
    builder.leaf(SyntaxKind::Operation, "-".into());
    builder.leaf(SyntaxKind::Number, "4".into());

    builder.finish_internal();

    let node = builder.finish_mut();

    let group = node.first_child().unwrap();
    let paren = group.first_child().unwrap();
    let number = paren.next_sibling().unwrap();
    let op = number.next_sibling().unwrap();
    println!("{}", op);
    op.insert_after(SyntaxKind::Operation, Some("/".into()));
    op.remove();

    println!("{}", node);
}
