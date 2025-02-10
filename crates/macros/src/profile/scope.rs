//! XXX: See https://github.com/rust-lang/rust/issues/54727
//! This is useless until Rust supports proc macros on non declarations

use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{LitStr, Stmt};
use venial::Error;

use crate::AttributeMacro;

pub struct ProfileScope;

impl AttributeMacro<(String, Stmt)> for ProfileScope {
    fn parse_attribute(
        input: proc_macro::TokenStream,
        body: proc_macro::TokenStream,
    ) -> Result<(String, Stmt), venial::Error> {
        let name = syn::parse::<LitStr>(input).map_err(|err| Error::new(err))?;
        let name = name.value();
        let expr = syn::parse(body).map_err(|err| Error::new(err))?;

        Ok((name, expr))
    }

    fn expand((name, stmt): (String, Stmt)) -> Result<proc_macro2::TokenStream, venial::Error> {
        let (pre, post, out) = match stmt {
            Stmt::Local(local) => {
                let var_pat = &local.pat;
                let Some(var_content) = local.init else {
                    return Err(Error::new("Declarations are not supported"));
                };
                let var_content = var_content.expr;

                (
                    Some(quote_spanned! {var_pat.span() => let #var_pat = }),
                    Some(quote! {;}),
                    quote_spanned! {var_content.span() => #var_content},
                )
            }
            Stmt::Item(_) => return Err(Error::new("Declarations are not supported")),
            Stmt::Expr(expr, _) => (None, None, quote_spanned! {expr.span() => #expr}),
            Stmt::Macro(m) => (None, None, quote_spanned! {m.span() => #m}),
        };

        let exit = "exit in {:?}";
        Ok(quote! {
            #pre {
                let _profile_start = ::std::time::SystemTime::now();

                let out = #out;

                ::tracing::warn!(target: #name, #exit, duration = _profile_start.elapsed());

                out
            } #post
        })
    }
}
