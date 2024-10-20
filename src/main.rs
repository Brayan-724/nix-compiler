pub mod builtins;
mod expr;
mod scope;
mod value;

use std::env;

use scope::FileScope;

fn main() {
    let mut iter = env::args().skip(1).peekable();

    let Some(file) = iter.next() else {
        eprintln!("Usage: nix-compiler <file>");
        return;
    };

    let result = FileScope::from_path(file).evaluate().unwrap();
    let result = result.as_ref().borrow();

    println!("Result (Expanded): {result:#}");
    println!("Result (Minimized): {result:}");
}
