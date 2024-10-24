pub mod builtins;
mod expr;
pub mod flake;
mod result;
mod scope;
mod value;

use std::env;

pub use builtins::NixValueBuiltin;
pub use result::{NixError, NixResult};
pub use scope::{FileScope, Scope};
pub use value::{
    AsAttrSet, AsString, LazyNixValue, NixLambdaParam, NixValue, NixValueWrapped, NixVar,
};

fn main() {
    let mut iter = env::args().skip(1).peekable();

    let Some(file) = iter.next() else {
        eprintln!("Usage: nix-compiler <file>");
        return;
    };

    let is_flake = file.ends_with("flake.nix");

    let result = FileScope::from_path(file).evaluate().unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    if is_flake {
        let outputs = flake::resolve_flake(result).unwrap();

        let outputs = LazyNixValue::Concrete(outputs)
            .wrap_var()
            .resolve_set(true)
            .unwrap_or_else(|err| {
                eprintln!("{err}");
                std::process::exit(1);
            });

        println!("Result (Expanded): {:#}", outputs.borrow());
        println!("Result (Minimized): {}", outputs.borrow());
    } else {
        println!("Result (Expanded): {:#}", result.borrow());
        println!("Result (Minimized): {}", result.borrow());
    }
}
