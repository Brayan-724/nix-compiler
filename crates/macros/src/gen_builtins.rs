use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::builtin::get_builtins;

pub struct GetBuiltins;

impl super::ProcMacro for GetBuiltins {
    type Item = TokenStream;

    fn parse(input: proc_macro::TokenStream) -> Result<TokenStream, venial::Error> {
        Ok(input.into())
    }

    fn expand(input: TokenStream) -> Result<TokenStream, venial::Error> {
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
}
