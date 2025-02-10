use quote::quote;
use syn::{Expr, LitStr};
use venial::Error;

use crate::AttributeMacro;

pub struct ProfileScope;

impl AttributeMacro<(String, Expr)> for ProfileScope {
    fn parse_attribute(
        input: proc_macro::TokenStream,
        body: proc_macro::TokenStream,
    ) -> Result<(String, Expr), venial::Error> {
        let name = syn::parse::<LitStr>(input).map_err(|err| Error::new(err))?;
        let name = name.value();
        let expr = syn::parse(body).map_err(|err| Error::new(err))?;

        Ok((name, expr))
    }

    fn expand((name, expr): (String, Expr)) -> Result<proc_macro2::TokenStream, venial::Error> {
        let exit = "exit in {:?}";

        Ok(quote! {
            let _profile_start = ::std::time::SystemTime::now();

            #expr

            ::tracing::warn!(target: #name, #exit, duration = _profile_start.elapsed());
        })
    }
}
