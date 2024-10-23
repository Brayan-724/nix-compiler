pub mod builtins;
mod expr;
pub mod flake;
mod scope;
mod value;

use std::env;

use scope::FileScope;
use value::LazyNixValue;

fn main() {
    let mut iter = env::args().skip(1).peekable();

    let Some(file) = iter.next() else {
        eprintln!("Usage: nix-compiler <file>");
        return;
    };

    let is_flake = file.ends_with("flake.nix");

    let result = FileScope::from_path(file).evaluate().unwrap();

    if is_flake {
        let outputs = flake::resolve_flake(result);

        let outputs = LazyNixValue::Concrete(outputs).wrap_var().resolve_set(true);

        println!("Result (Expanded): {:#}", outputs.borrow());
        println!("Result (Minimized): {}", outputs.borrow());
    } else {
        println!("Result (Expanded): {:#}", result.borrow());
        println!("Result (Minimized): {}", result.borrow());
    }
}
