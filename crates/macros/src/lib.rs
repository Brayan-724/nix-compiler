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
pub fn gen_builtins(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    gen_builtins_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn gen_builtins_impl(input: TokenStream) -> Result<TokenStream, Error> {
    let builtins = get_builtins()?
        .split(";")
        .map(|builtin| {
            let builtin = format_ident!("{builtin}");

            quote! { builtins.insert(<#builtin as crate::builtins::NixBuiltinInfo>::NAME.to_owned(), #builtin::generate().wrap_var()) }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        pub fn get_builtins() -> NixValue {
            let mut builtins = crate::NixAttrSet::new();

            #(#builtins;)*

            {
                macro_rules! insert {
                    ($($name:ident = $value:expr);* $(;)?) => {
                        $(
                        builtins.insert(stringify!($name).to_owned(), $value.wrap_var());
                        )*
                    }
                }

                insert!(
                    #input
                );
            }

            NixValue::AttrSet(builtins)
        }
    })
}

#[proc_macro_attribute]
pub fn profile(
    _: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let func = syn::parse_macro_input!(body as syn::ItemFn);

    profile_impl(func)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn profile_impl(func: syn::ItemFn) -> Result<TokenStream, Error> {
    let func_attrs = &func.attrs;
    let func_vis = &func.vis;
    let func_ident = &func.sig.ident;
    let func_args = &func.sig.inputs;
    let func_ret = &func.sig.output;
    let func_body = &func.block;

    let exit = "exit in {:?}";

    Ok(quote! {
        #(#func_attrs)*
        #func_vis fn #func_ident(#func_args) #func_ret {
            let start = ::std::time::SystemTime::now();

            let output = move || {
                let __span = ::tracing::warn_span!(stringify!(#func_ident));
                let __span = __span.enter();

                #func_body
            };
            let output = output();

            ::tracing::warn!(target: stringify!(#func_ident), #exit, duration = start.elapsed());

            output
        }
    })
}
