mod expr;
mod scope;
mod value;

use std::{env, fs};

use scope::Scope;

fn main() {
    let mut iter = env::args().skip(1).peekable();
    if iter.peek().is_none() {
        eprintln!("Usage: nix-compiler <file>");
        return;
    }
    for file in iter {
        let content = match fs::read_to_string(file) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("error reading file: {}", err);
                return;
            }
        };
        let parse = rnix::Root::parse(&content);

        for error in parse.errors() {
            println!("error: {}", error);
        }

        let root = parse.tree();
        // println!("{:#?}", root);

        let root = root.expr().unwrap();

        let scope = Scope::new();

        let result = scope.visit_expr(root);
        let result = result.as_ref().borrow();

        println!("Result (Expanded): {result:#}");
        println!("Result (Minimized): {result:}");

        println!("{scope:#?}");
    }
}
