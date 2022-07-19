use proc_macro::TokenStream;

mod bitflags;
mod derive;

synstructure::decl_derive! {
    [WriteUrl, attributes(wp)] =>
    /// derives either `WriteUrlValue` or `WriteUrlParams`, depending on the configuration.
    /// 
    /// `WriteUrlParams`: writes to an url writer with param name and value pairs.
    /// 
    /// `WriteUrlValue`: writes to an url writer, with a primary value with param name given,
    /// and may write additional name and value pairs.
    ///
    /// by default, field names are converted to lowercase, since this is not distinguished in
    /// the api.
    /// 
    /// #### `struct`s
    /// 
    /// Structs can only derive `WriteUrlParams`. Their field names will be used for the url
    /// param name.
    /// 
    /// #### `enum`s
    /// 
    /// Enums by default have a surprising implementation: The name of a variant will be used
    /// to serialize as the url `value`. It is also possible to have multiple enums in an `EnumSet`.
    /// Values contained in the enum will be serialized as futher `key=value` pairs. This will implement
    /// `WriteUrlValue` by default.
    /// 
    /// #### `#[wp(flatten)]`
    /// 
    /// Applying flatten to a field will cause its name to be discarded by the macro.
    /// The field's `WriteUrlParams` implementation will be invoked instead of `WriteUrlValue`.
    /// 
    /// #### `#[wp(prepend_all = "xxx")]`
    /// 
    /// Applying prepend_all with a string value will cause that string to be prepended to the names for
    /// the query. This is compatible with `flatten`: An adaptor will be used to ensure that all futher
    /// writes will be prepended.
    ///
    /// #### `#[wp(name = "xxx")]`
    /// 
    /// Applying this on a field will completely override the name used while writing the field. This will
    /// ignore `prepend_all`.
    /// 
    /// #### `#[wp(unnamed)]`
    /// 
    /// When applied to an enum, the enum variants' names are discarded, and fields are serialized as `key=value`
    /// pairs. This will implement `WriteUrlParams` for an enum.
    /// 
    /// #### `#[wp(mutual_exclusive)]`
    /// 
    /// When applied to an enum, the enum variants' names will be used as keys for `key=value` pairs. All variants
    /// must be a newtype variant with a type that implements `WriteUrlValue`. This will implement `WriteUrlParams`
    /// for an enum.
    derive::derive_write_url
}

#[proc_macro]
pub fn bitflags(input: TokenStream) -> TokenStream {
    bitflags::bitflags(input.into())
        .unwrap_or_else(|t| t.into_compile_error())
        .into()
}
