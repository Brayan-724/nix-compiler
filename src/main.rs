pub mod builtins;
mod expr;
pub mod flake;
mod scope;
mod value;

use std::env;
use std::ops::Deref;

use scope::FileScope;
use value::{AsAttrSet, LazyNixValue};

fn main() {
    let mut iter = env::args().skip(1).peekable();

    let Some(file) = iter.next() else {
        eprintln!("Usage: nix-compiler <file>");
        return;
    };

    let is_flake = file.ends_with("flake.nix");

    let result = FileScope::from_path(file).evaluate().unwrap();

    println!("Result (Expanded): {result:#?}");
    println!("Result (Minimized): {result:?}");

    if is_flake {
        let outputs = flake::resolve_flake(result);

        let outputs = outputs.resolve_set(true);
        println!("END: {:#}", outputs.borrow());
    }
}
