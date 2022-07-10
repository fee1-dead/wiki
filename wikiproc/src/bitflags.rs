use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::Parse;
use syn::token::Brace;
use syn::{braced, parse2, Expr, Token, Visibility};

pub fn bitflags(input: TokenStream) -> syn::Result<TokenStream> {
    let tts = input.clone();
    let BitflagsInput { name, fields, .. } = parse2(input)?;
    let fields = fields.into_iter().map(|Bitfield { name, .. }| {
        let value = name
            .to_string()
            .chars()
            .filter(|c| *c != '_')
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>();
        quote! {
            if self.contains(Self::#name) {
                encoder__.push(#value);
            }
        }
    });
    Ok(quote! {
        ::bitflags::bitflags! {
            #tts
        }
        impl ::wiki::macro_support::WriteUrlValue for #name {
            fn ser<W: ::wiki::macro_support::UrlParamWriter>(
                &self,
                w__: ::wiki::macro_support::BufferedName<'_, W>,
            ) -> ::core::result::Result<(), W::E>
            {
                let mut encoder__ = ::wiki::macro_support::MultiValueEncoder::new(false);
                #(#fields)*
                w__.write(::wiki::macro_support::TriStr::Owned(encoder__.build()))?;
                Ok(())
            }
        }
    })
}

pub struct BitflagsInput {
    pub vis: Visibility,
    pub struct_: Token![struct],
    pub name: Ident,
    pub colon: Token![:],
    pub ty: Ident,
    pub brace: Brace,
    pub fields: Vec<Bitfield>,
}

impl Parse for BitflagsInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let vis = input.parse()?;
        let struct_ = input.parse()?;
        let name = input.parse()?;
        let colon = input.parse()?;
        let ty = input.parse()?;
        let content;
        let brace = braced!(content in input);
        let mut fields = vec![];
        while !content.is_empty() {
            fields.push(content.parse()?);
        }
        Ok(Self {
            vis,
            struct_,
            name,
            colon,
            ty,
            brace,
            fields,
        })
    }
}

pub struct Bitfield {
    pub const_: Token![const],
    pub name: Ident,
    pub eq: Token![=],
    pub exp: Expr,
    pub semi: Token![;],
}

impl Parse for Bitfield {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            const_: input.parse()?,
            name: input.parse()?,
            eq: input.parse()?,
            exp: input.parse()?,
            semi: input.parse()?,
        })
    }
}
