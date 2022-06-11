use proc_macro2::TokenStream as Ts;
use quote::quote;
use syn::{
    spanned::Spanned,
    Data, Fields, FieldsUnnamed, Lit, Meta, MetaNameValue, NestedMeta,
};
use synstructure::VariantInfo;

synstructure::decl_derive!([WriteUrl, attributes(wikiproc)] => derive_write_url);

#[derive(Default)]
struct Options {
    named: Option<bool>,
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
        Ok(r)
    }
}

fn gen_fields(v: &VariantInfo) -> Ts {
    match v.ast().fields {
        Fields::Named(_) => v
            .bindings()
            .iter()
            .map(|b| {
                let mut opts = FieldOptions::default();
                for a in &b.ast().attrs {
                    if a.path.is_ident("wikiproc") {
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
                if opts.flatten {
                    quote!({crate::WriteUrlParams::ser(#b, w)?;})
                } else {
                    let name = opts.override_name.unwrap_or_else(|| {
                        b.ast()
                            .ident
                            .as_ref()
                            .unwrap()
                            .to_string()
                            .to_ascii_lowercase()
                    });
                    quote! {{
                        let n = w.fork(crate::TriStr::Static(#name));
                        crate::WriteUrlValue::ser(#b, n)?;
                    }}
                }
            })
            .collect(),
        Fields::Unit => quote!(),
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.is_empty() => quote!(),
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.len() == 1 => v
            .bindings()
            .iter()
            .map(|i| quote!(crate::WriteUrlParams::ser(#i, w)?;))
            .collect(),
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => syn::Error::new_spanned(
            unnamed,
            "too many fields, use newtype or named fields instead",
        )
        .into_compile_error(),
    }
}

fn variant_name(v: &VariantInfo) -> String {
    let attr = v
        .ast()
        .attrs
        .iter()
        .filter(|a| a.path.is_ident("wikiproc"))
        .next();
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
            if path.is_ident("value") {
                return Some(s);
            }
        }
        None
    });
    let name = name
        .map(|s| s.value())
        .unwrap_or_else(|| v.ast().ident.to_string().to_ascii_lowercase());
    name
}

fn derive_write_url(s: synstructure::Structure) -> syn::Result<Ts> {
    let mut opts = Options::default();
    for attr in &s.ast().attrs {
        if attr.path.get_ident().map_or(false, |i| i == "wikiproc") {
            let m = attr.parse_meta()?;
            opts = Options::parse(m)?;
        }
    }

    match s.ast().data {
        Data::Union(_) => Err(syn::Error::new(s.ast().span(), "data union not supported")),
        Data::Struct(_) => {
            let body = s.each_variant(gen_fields);
            Ok(s.gen_impl(quote::quote! {
                gen impl crate::WriteUrlParams for @Self {
                    fn ser<W_: crate::UrlParamWriter>(&self, w: &mut W_) -> ::std::result::Result<(), W_::E> {
                        match *self { #body }
                        Ok(())
                    }
                }
            }))
        }
        Data::Enum(_) => {
            if opts.named.unwrap_or(true) {
                let body = s.each_variant(gen_fields);
                let vnames = s.each_variant(variant_name);
                let a = s.gen_impl(quote! {
                    gen impl crate::WriteUrlValue for @Self {
                        fn ser<W_: crate::UrlParamWriter>(&self, w: crate::BufferedName<'_, W_>) -> ::std::result::Result<(), W_::E> {
                            let w = w.write(crate::TriStr::Static(crate::NamedEnum::variant_name(self)))?;
                            self.ser_additional_only(w)
                        }
                        fn ser_additional_only<W_: crate::UrlParamWriter>(&self, w: &mut W_) -> ::std::result::Result<(), W_::E> {
                            match *self { #body }
                            Ok(())
                        }
                    }
                });
                let b = s.gen_impl(quote! {
                    gen impl crate::NamedEnum for @Self {
                        fn variant_name(&self) -> &'static str {
                            match *self { #vnames }
                        }
                    }
                });
                Ok(quote! { #a #b })
            } else {
                let body = s.each_variant(|v| match v.ast().fields {
                    Fields::Named(_) => v
                        .bindings()
                        .iter()
                        .map(|b| {
                            let name = b.ast().ident.as_ref().unwrap().to_string();
                            quote! {{
                                let n = w.fork(crate::TriStr::Static(#name));
                                crate::WriteUrlValue::ser(#b, n)?;
                            }}
                        })
                        .collect(),
                    Fields::Unnamed(FieldsUnnamed { unnamed, .. }) if unnamed.len() == 1 => v
                        .bindings()
                        .iter()
                        .map(|i| quote!(crate::WriteUrlParams::ser(#i, w)?;))
                        .collect(),
                    Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => syn::Error::new_spanned(
                        unnamed,
                        "too many fields, use newtype or named fields instead",
                    )
                    .into_compile_error(),
                    Fields::Unit => quote!(),
                });
                let i = s.gen_impl(quote::quote! {
                    gen impl crate::WriteUrlParams for @Self {
                        fn ser<W_: crate::UrlParamWriter>(&self, w: &mut W_) -> ::std::result::Result<(), W_::E> {
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
