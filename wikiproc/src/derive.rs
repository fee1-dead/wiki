use proc_macro2::{Span, TokenStream as Ts};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, Fields, FieldsUnnamed, Lit, LitInt, Meta, MetaNameValue, NestedMeta};
use synstructure::VariantInfo;

#[derive(Default)]
struct Options {
    named: Option<bool>,
    prepend_all: Option<String>,
    mutual_exclusive: bool,
    // default_lowercase: bool,
}

#[derive(Default)]
struct FieldOptions {
    flatten: bool,
    override_name: Option<String>,
}

impl Options {
    pub fn parse(meta: Meta) -> syn::Result<Self> {
        let mut r = Self::default();
        match meta {
            Meta::List(ml) => {
                for m in ml.nested {
                    match m {
                        NestedMeta::Meta(Meta::Path(p)) if p.is_ident("named") => {
                            r.named = Some(true)
                        }
                        NestedMeta::Meta(Meta::Path(p)) if p.is_ident("unnamed") => {
                            r.named = Some(false)
                        }
                        NestedMeta::Meta(Meta::Path(p)) if p.is_ident("mutual_exclusive") => {
                            r.mutual_exclusive = true;
                        }
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                            path,
                            lit: Lit::Str(s),
                            ..
                        })) if path.is_ident("prepend_all") => {
                            r.prepend_all = Some(s.value());
                        }
                        _ => return Err(syn::Error::new_spanned(m, "invalid meta")),
                    }
                }
            }
            _ => return Err(syn::Error::new_spanned(meta, "invalid options")),
        }
        Ok(r)
    }
}

impl FieldOptions {
    pub fn parse(meta: Meta) -> syn::Result<Self> {
        let mut r = Self::default();
        let span = meta.span();
        match meta {
            Meta::List(ml) => {
                for m in ml.nested {
                    match m {
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                            path,
                            lit: syn::Lit::Str(s),
                            ..
                        })) if path.is_ident("name") => r.override_name = Some(s.value()),
                        NestedMeta::Meta(Meta::Path(p)) if p.is_ident("flatten") => {
                            r.flatten = true
                        }
                        _ => return Err(syn::Error::new_spanned(m, "invalid meta")),
                    }
                }
            }
            _ => return Err(syn::Error::new_spanned(meta, "invalid options")),
        }
        r.verify(span)?;
        Ok(r)
    }

    pub fn verify(&self, s: Span) -> syn::Result<()> {
        Err(syn::Error::new(
            s,
            match self {
                FieldOptions {
                    flatten: true,
                    override_name: Some(_),
                    ..
                } => "`flatten` and `name` are mutually exclusive",
                _ => return Ok(()),
            },
        ))
    }
}

fn gen_fields(v: &VariantInfo, o: &Options) -> Ts {
    match v.ast().fields {
        Fields::Named(_) => v
            .bindings()
            .iter()
            .map(|b| {
                let mut opts = FieldOptions::default();
                for a in &b.ast().attrs {
                    if a.path.is_ident("wp") {
                        let m = match a.parse_meta() {
                            Ok(m) => m,
                            Err(e) => return e.into_compile_error(),
                        };
                        opts = match FieldOptions::parse(m) {
                            Ok(opts) => opts,
                            Err(e) => return e.into_compile_error(),
                        };
                    }
                }
                match (&o.prepend_all, opts) {
                    (Some(pp), FieldOptions { flatten: true, .. }) => {
                        quote!({::wiki::macro_support::WriteUrlParams::ser(#b, &mut ::wiki::macro_support::PrependAdaptor::new(&mut w, #pp))?;})
                    }
                    (None, FieldOptions { flatten: true, .. }) => {
                        quote!({::wiki::macro_support::WriteUrlParams::ser(#b, w)?;})
                    }
                    (pp, FieldOptions { override_name, .. }) => {
                        let name = override_name.unwrap_or_else(|| {
                            let mut s = pp.clone().unwrap_or_default();
                            s.push_str(
                                &*b.ast()
                                    .ident
                                    .as_ref()
                                    .unwrap()
                                    .to_string()
                                    .to_ascii_lowercase(),
                            );
                            s
                        });
                        quote! {{
                            let n = w.fork(::wiki::macro_support::TriStr::Static(#name));
                            ::wiki::macro_support::WriteUrlValue::ser(#b, n)?;
                        }}
                    }
                }
            })
            .collect(),
        Fields::Unit => quote!(),
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.is_empty() => quote!(),
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.len() == 1 => v
            .bindings()
            .iter()
            .map(|i| quote!(::wiki::macro_support::WriteUrlParams::ser(#i, w)?;))
            .collect(),
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => syn::Error::new_spanned(
            unnamed,
            "too many fields, use newtype or named fields instead",
        )
        .into_compile_error(),
    }
}

fn variant_name(v: &VariantInfo) -> String {
    let attr = v.ast().attrs.iter().find(|a| a.path.is_ident("wp"));
    let name = attr.and_then(|a| a.parse_meta().ok()).and_then(|m| {
        let m = if let Meta::List(l) = m {
            l.nested.into_iter().next()?
        } else {
            return None;
        };
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
            path,
            lit: Lit::Str(s),
            ..
        })) = m
        {
            if path.is_ident("name") {
                return Some(s);
            }
        }
        None
    });

    name.map(|s| s.value())
        .unwrap_or_else(|| v.ast().ident.to_string().to_ascii_lowercase())
}

pub fn derive_write_url(s: synstructure::Structure) -> syn::Result<Ts> {
    let mut opts = Options::default();
    for attr in &s.ast().attrs {
        if attr.path.get_ident().map_or(false, |i| i == "wp") {
            let m = attr.parse_meta()?;
            opts = Options::parse(m)?;
        }
    }

    match s.ast().data {
        Data::Union(_) => Err(syn::Error::new(s.ast().span(), "data union not supported")),
        Data::Struct(_) => {
            let body = s.each_variant(|v| gen_fields(v, &opts));
            Ok(s.gen_impl(quote::quote! {
                gen impl ::wiki::macro_support::WriteUrlParams for @Self {
                    fn ser<W_: ::wiki::macro_support::UrlParamWriter>(&self, mut w: &mut W_) -> ::std::result::Result<(), W_::E> {
                        match *self { #body }
                        Ok(())
                    }
                }
            }))
        }
        Data::Enum(_) => {
            if opts.mutual_exclusive {
                let body = s.each_variant(|v| match v.ast().fields {
                    Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.len() == 1 => {
                        let name = variant_name(v);
                        let binding = &v.bindings()[0];

                        quote! {{
                            let b = w.fork(::wiki::macro_support::TriStr::Static(#name));
                            ::wiki::macro_support::WriteUrlValue::ser(#binding, b)?;
                        }}
                    }
                    _ => syn::Error::new_spanned(
                        v.ast().ident,
                        "too many fields, use newtype or named fields instead",
                    )
                    .into_compile_error(),
                });
                let i = s.gen_impl(quote::quote! {
                    gen impl ::wiki::macro_support::WriteUrlParams for @Self {
                        fn ser<W_: ::wiki::macro_support::UrlParamWriter>(&self, w: &mut W_) -> ::std::result::Result<(), W_::E> {
                            match *self { #body }
                            Ok(())
                        }
                    }
                });
                Ok(i)
            } else if opts.named.unwrap_or(true) {
                let body = s.each_variant(|v| gen_fields(v, &opts));
                let vnames = s.each_variant(variant_name);
                let ty = match s.variants().len() {
                    0..=8 => quote!(u8),
                    9..=16 => quote!(u16),
                    17..=32 => quote!(u32),
                    33..=64 => quote!(u64),
                    _ => panic!("too many variants"),
                };
                let mut i: u64 = 1;
                let vnums = s.each_variant(|_| {
                    let n = i;
                    i <<= 1;
                    LitInt::new(&format!("{n}"), Span::mixed_site())
                });
                let a = s.gen_impl(quote! {
                    gen impl ::wiki::macro_support::WriteUrlValue for @Self {
                        fn ser<W_: ::wiki::macro_support::UrlParamWriter>(&self, w: ::wiki::macro_support::BufferedName<'_, W_>) -> ::std::result::Result<(), W_::E> {
                            let w = w.write(::wiki::macro_support::TriStr::Static(::wiki::macro_support::NamedEnum::variant_name(self)))?;
                            self.ser_additional_only(w)
                        }
                        fn ser_additional_only<W_: ::wiki::macro_support::UrlParamWriter>(&self, w: &mut W_) -> ::std::result::Result<(), W_::E> {
                            match *self { #body }
                            Ok(())
                        }
                    }
                });
                let b = s.gen_impl(quote! {
                    gen impl ::wiki::macro_support::BitflaggedEnum for @Self {
                        type Bitflag = #ty;
                        fn flag(&self) -> Self::Bitflag {
                            match *self { #vnums }
                        }
                    }
                });
                let c = s.gen_impl(quote! {
                    gen impl ::wiki::macro_support::NamedEnum for @Self {
                        fn variant_name(&self) -> &'static str {
                            match *self { #vnames }
                        }
                    }
                });
                Ok(quote! { #a #b #c })
            } else {
                let body = s.each_variant(|v| match v.ast().fields {
                    Fields::Named(_) => v
                        .bindings()
                        .iter()
                        .map(|b| {
                            let name = b.ast().ident.as_ref().unwrap().to_string();
                            quote! {{
                                let n = w.fork(::wiki::macro_support::TriStr::Static(#name));
                                ::wiki::macro_support::WriteUrlValue::ser(#b, n)?;
                            }}
                        })
                        .collect(),
                    Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.len() == 1 => v
                        .bindings()
                        .iter()
                        .map(|i| quote!(::wiki::macro_support::WriteUrlParams::ser(#i, w)?;))
                        .collect(),
                    Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => syn::Error::new_spanned(
                        unnamed,
                        "too many fields, use newtype or named fields instead",
                    )
                    .into_compile_error(),
                    Fields::Unit => quote!(),
                });
                let i = s.gen_impl(quote::quote! {
                    gen impl ::wiki::macro_support::WriteUrlParams for @Self {
                        fn ser<W_: ::wiki::macro_support::UrlParamWriter>(&self, w: &mut W_) -> ::std::result::Result<(), W_::E> {
                            match *self { #body }
                            Ok(())
                        }
                    }
                });
                Ok(i)
            }
        }
    }
}
