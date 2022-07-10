use proc_macro::TokenStream;

mod bitflags;
mod derive;

synstructure::decl_derive!([WriteUrl, attributes(wp)] => derive::derive_write_url);

#[proc_macro]
pub fn bitflags(input: TokenStream) -> TokenStream {
    bitflags::bitflags(input.into())
        .unwrap_or_else(|t| t.into_compile_error())
        .into()
}
