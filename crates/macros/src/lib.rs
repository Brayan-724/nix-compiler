mod builtin;
mod params;

use builtin::{get_builtins, Builtin};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use venial::{parse_item, Error, Item};

#[proc_macro_attribute]
pub fn builtin(
    _: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let func = match parse_item(body.into()) {
        Err(e) => Err(e),
        Ok(Item::Function(func)) => Ok(func),
        Ok(_) => Err(Error::new("")),
    };

    func.and_then(Builtin::new)
        .and_then(Builtin::generate)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro]
pub fn gen_builtins(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    gen_builtins_impl()
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn gen_builtins_impl() -> Result<TokenStream, Error> {
    let builtins = get_builtins()?
        .split(";")
        .map(|builtin| {
            let builtin = format_ident!("{builtin}");

            quote! { builtins.insert(<#builtin as crate::builtins::NixBuiltinInfo>::NAME.to_owned(), #builtin::generate().wrap_var()) }
        })
        .collect::<Vec<_>>();

    let nix_version_key = "nixVersion";
    let nix_version = "2.24.9";

    Ok(quote! {
        pub fn get_builtins() -> NixValue {
            use std::collections::HashMap;
            use crate::NixValue;

            let mut builtins = HashMap::new();

            builtins.insert(
                #nix_version_key.to_owned(),
                NixValue::String(String::from(#nix_version)).wrap_var(),
            );

            #(#builtins;)*

            NixValue::AttrSet(builtins)
        }
    })
}
