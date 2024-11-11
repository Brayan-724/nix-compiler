pub mod builtins;
mod expr;
pub mod flake;
mod result;
mod scope;
mod value;

use std::env;

pub use builtins::{NixBuiltin, NixBuiltinInfo};
pub use result::{
    NixBacktrace, NixError, NixLabel, NixLabelKind, NixLabelMessage, NixResult, NixSpan,
};
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

    let (backtrace, result) = FileScope::from_path(file)
        .evaluate(None)
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });

    let outputs = if is_flake {
        flake::resolve_flake(backtrace.clone(), result).unwrap()
    } else {
        result
    };

    let outputs = LazyNixValue::Concrete(outputs)
        .wrap_var()
        .resolve_set(true, backtrace)
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });

    println!("Result (Expanded): {:#}", outputs.borrow());
    println!("Result (Minimized): {}", outputs.borrow());
}
