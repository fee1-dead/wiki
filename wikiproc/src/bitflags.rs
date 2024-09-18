use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::Parse;
use syn::token::Brace;
use syn::{braced, parse2, Attribute, Expr, Token, Visibility};

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
    pub _attrs: Vec<Attribute>,
    pub _vis: Visibility,
    pub _struct: Token![struct],
    pub name: Ident,
    pub _colon: Token![:],
    pub _ty: Ident,
    pub _brace: Brace,
    pub fields: Vec<Bitfield>,
}

impl Parse for BitflagsInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let _attrs = input.call(Attribute::parse_outer)?;
        let _vis = input.parse()?;
        let _struct = input.parse()?;
        let name = input.parse()?;
        let _colon = input.parse()?;
        let _ty = input.parse()?;
        let content;
        let _brace = braced!(content in input);
        let mut fields = vec![];
        while !content.is_empty() {
            fields.push(content.parse()?);
        }
        Ok(Self {
            _attrs,
            _vis,
            _struct,
            name,
            _colon,
            _ty,
            _brace,
            fields,
        })
    }
}

pub struct Bitfield {
    pub _const: Token![const],
    pub name: Ident,
    pub _eq: Token![=],
    pub _exp: Expr,
    pub _semi: Token![;],
}

impl Parse for Bitfield {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            _const: input.parse()?,
            name: input.parse()?,
            _eq: input.parse()?,
            _exp: input.parse()?,
            _semi: input.parse()?,
        })
    }
}
