use quote::quote;
use syn::ItemFn;
use venial::Error;

use crate::AttributeMacro;

pub struct Profile;

impl AttributeMacro<ItemFn> for Profile {
    fn parse_attribute(
        _: proc_macro::TokenStream,
        body: proc_macro::TokenStream,
    ) -> Result<ItemFn, venial::Error> {
        syn::parse(body).map_err(|err| Error::new(err))
    }

    fn expand(func: ItemFn) -> Result<proc_macro2::TokenStream, venial::Error> {
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
}
