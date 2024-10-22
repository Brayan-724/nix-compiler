pub mod builtins;
mod expr;
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
        let result = result.resolve();
        let result = result.borrow();

        let Some(flake) = result.as_attr_set() else {
            todo!("Error handling");
        };

        let outputs = flake.get("outputs").expect("Flake should export `outputs`");
        let outputs = outputs.resolve();

        println!("{outputs:#?}");

        let outputs = outputs.borrow();
        let Some((scope, param, expr)) = outputs.as_lambda() else {
            todo!("outputs should be a lambda")
        };

        let scope = scope.clone().new_child();
        let outputs = LazyNixValue::Pending(scope.clone(), expr.clone()).wrap_var();

        scope.set_variable("self".to_owned(), outputs.clone());

        let outputs = outputs.resolve_set(true);
        println!("{:#}", outputs.borrow());
    }
}
