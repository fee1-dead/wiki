use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Deserializer};
use wikiproc::WriteUrl;

use crate::req::Format;
use crate::url::{BufferedName, TriStr, UrlParamWriter, WriteUrlValue};

pub trait Query {
    type Output: for<'de> Deserialize<'de>;
}

mod sealed {
    use super::{Query, UsizeBool};
    use crate::url::{WriteUrlParams, WriteUrlValue};

    pub trait Action: WriteUrlValue + Query {}
    pub trait Main: WriteUrlParams + Query {}
    impl<const A: usize, const B: usize, T: Action> Main for super::Main<A, B, T>
    where
        (): UsizeBool<A> + UsizeBool<B>,
        T: WriteUrlParams,
    {
    }
    impl<T, const A: usize, const B: usize, const C: usize> Action for super::action::Parse<T, A, B, C>
    where
        (): UsizeBool<A> + UsizeBool<B> + UsizeBool<C>,
        T: WriteUrlParams,
    {
    }
}
pub use s::Main as IsMain;
use sealed as s;

pub mod action {
    use serde::Deserialize;
    use wiki::url::{BufferedName, UrlParamWriter, WriteUrlValue};
    use wikiproc::WriteUrl;

    use super::{Optional, Query, UsizeBool, UsizeBool as U};
    use crate::req::MultiValueEncoder;
    use crate::url::{TriStr, WriteUrlParams};

    /// select an existing page/revision
    #[derive(WriteUrl)]
    pub struct PageTitleSelector {
        pub page: String,
    }

    #[derive(WriteUrl)]
    pub struct PageIdSelector {
        pub pageid: u64,
    }

    #[derive(WriteUrl)]
    pub struct RevIdSelector {
        pub oldid: u64,
    }

    // OR relationship
    pub trait ContentModel<const A: usize, const B: usize> {}

    impl ContentModel<1, 1> for () {}
    impl ContentModel<0, 1> for () {}
    impl ContentModel<1, 0> for () {}

    #[derive(WriteUrl)]
    pub struct ExplicitContentSelector<
        const REVID: usize,
        const TITLE: usize,
        const CONTENTMODEL: usize,
    >
    where
        (): UsizeBool<REVID>
            + UsizeBool<TITLE>
            + UsizeBool<CONTENTMODEL>
            + ContentModel<TITLE, CONTENTMODEL>,
    {
        pub revid: Optional<REVID, u64>,
        pub title: Optional<TITLE, String>,
        // TODO contentmodel
        pub contentmodel: Optional<CONTENTMODEL, String>,
    }

    macro_rules! const_generics_flags {
        (@gen_impls($Struct:ident, {$($before:ident)*})) => {};
        (@gen_impls($Struct:ident, {$($before:ident)*}, {$curname:ident, $curwith:ident} $(, {$nextname:ident, $nextwith:ident})*)) => {
            impl<$(const $before: usize,)* const $curname: usize, $(const $nextname: usize),*> $Struct<$($before,)* $curname, $($nextname),*> {
                pub fn $curwith(self) -> $Struct<$($before,)* 1, $($nextname),*> {
                    $Struct
                }
            }
            const_generics_flags!(@gen_impls($Struct, {$($before)* $curname}$(,{$nextname, $nextwith})*));
        };
        ($(#[$meta:meta])* pub struct $Struct:ident { $($NAME:ident = { $value:literal, $with:ident$(,)? }),*$(,)? }) => {
            $(#[$meta])*
            pub struct $Struct<$(const $NAME: usize),*>;
            const_generics_flags!(@gen_impls($Struct, {}, $({$NAME, $with}),*));

            impl<$(const $NAME: usize),*> WriteUrlValue for $Struct<$($NAME),*> {
                fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
                    let mut encoder = MultiValueEncoder::new(false);
                    $(
                        if $NAME != 0 {
                            encoder.push($value);
                        }
                    )*
                    w.write(TriStr::Owned(encoder.build()))?;
                    Ok(())
                }
            }
        };
    }

    const_generics_flags! {
        /// # Example
        /// ```
        /// use wiki::deterministic::action::ParseProps;
        /// let props: ParseProps<0, 0> = ParseProps;
        /// let props = props.with_text();
        /// let props = props.with_links();
        /// ```
        pub struct ParseProps {
            TEXT = {
                "text",
                with_text,
            },
            LINKS = {
                "links",
                with_links,
            },
        }
    }

    #[derive(WriteUrl)]
    pub struct Parse<Selector, const SUMMARY: usize, const TEXT: usize, const MODULES: usize>
    where
        (): U<SUMMARY> + U<TEXT> + U<MODULES>,
    {
        #[wp(flatten)]
        pub selector: Selector,
        pub summary: Optional<SUMMARY, String>,
        pub props: ParseProps<TEXT, MODULES>,
    }

    impl<T> Parse<T, 0, 0, 0> {
        pub fn new<
            F: FnOnce(ParseProps<0, 0>) -> ParseProps<A, B>,
            const A: usize,
            const B: usize,
        >(
            x: T,
            _func: F,
        ) -> Parse<T, 0, A, B>
        where
            (): UsizeBool<A> + UsizeBool<B>,
        {
            let parse_props: ParseProps<A, B> = ParseProps;
            Parse {
                selector: x,
                summary: Optional::none(),
                props: parse_props,
            }
        }
    }

    impl<T: WriteUrlParams, const A: usize, const B: usize, const C: usize> WriteUrlValue
        for Parse<T, A, B, C>
    where
        (): U<A> + U<B> + U<C>,
    {
        fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
            let w = w.write(TriStr::Static("parse"))?;
            self.ser_additional_only(w)
        }
        fn ser_additional_only<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E> {
            <Self as WriteUrlParams>::ser(self, w)
        }
    }

    impl<T, const A: usize, const B: usize, const C: usize> Query for Parse<T, A, B, C>
    where
        (): U<A> + U<B> + U<C>,
    {
        type Output = ParseResponse<B, C>;
    }

    #[derive(Deserialize)]
    pub struct ParseResponseInner<const TEXT: usize, const MODULES: usize>
    where
        (): UsizeBool<TEXT> + UsizeBool<MODULES>,
    {
        pub title: String,
        pub pageid: u64,
        pub text: Optional<TEXT, String>,
        pub modules: Optional<MODULES, Vec<String>>,
        pub modulescripts: Optional<MODULES, Vec<String>>,
        pub modulestyles: Optional<MODULES, Vec<String>>,
    }

    #[derive(Deserialize)]
    pub struct ParseResponse<const TEXT: usize, const MODULES: usize>
    where
        (): UsizeBool<TEXT> + UsizeBool<MODULES>,
    {
        pub parse: ParseResponseInner<TEXT, MODULES>,
    }
}

pub struct Action<T> {
    pub kind: T,
}

#[derive(WriteUrl)]
pub struct Main<const SERVEDBY: usize, const REQUESTID: usize, Action: s::Action>
where
    (): UsizeBool<SERVEDBY> + UsizeBool<REQUESTID>,
{
    servedby: Bool<SERVEDBY>,
    requestid: Optional<REQUESTID, String>,
    pub action: Action,
    format: Format,
}

impl<T: s::Action> Main<0, 0, T> {
    pub fn new(action: T) -> Self {
        Self {
            servedby: Bool,
            requestid: Optional::none(),
            action,
            format: Format::Json { formatversion: 2 },
        }
    }
}

impl<const SERVEDBY: usize, const REQUESTID: usize, Action: s::Action>
    Main<SERVEDBY, REQUESTID, Action>
where
    (): UsizeBool<SERVEDBY> + UsizeBool<REQUESTID>,
{
    pub fn servedby(self) -> Main<1, REQUESTID, Action> {
        Main {
            servedby: Bool,
            requestid: self.requestid,
            action: self.action,
            format: self.format,
        }
    }
    pub fn request_id(self, x: impl Into<String>) -> Main<SERVEDBY, 1, Action> {
        let opt = Optional::some(x.into());
        Main {
            servedby: self.servedby,
            requestid: opt,
            action: self.action,
            format: self.format,
        }
    }
    pub fn action<T: s::Action>(self, action: T) -> Main<SERVEDBY, REQUESTID, T> {
        Main {
            servedby: self.servedby,
            requestid: self.requestid,
            action,
            format: self.format,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct Response<
    const SERVEDBY: usize,
    const REQUESTID: usize,
    Action,
    // TODO const CURTIMESTAMP: usize
> where
    (): UsizeBool<SERVEDBY>,
    (): UsizeBool<REQUESTID>,
{
    pub servedby: Optional<SERVEDBY, String>,
    pub requestid: Optional<REQUESTID, String>,
    #[serde(flatten)]
    pub action: Action,
}

impl<const A: usize, const B: usize, Action: sealed::Action> Query for Main<A, B, Action>
where
    (): UsizeBool<A> + UsizeBool<B>,
{
    type Output = Response<A, B, Action::Output>;
}

pub struct Bool<const BOOL: usize>
where
    (): UsizeBool<BOOL>;

impl<const BOOL: usize> WriteUrlValue for Bool<BOOL>
where
    (): UsizeBool<BOOL>,
{
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        if BOOL == 1 {
            w.write(TriStr::Static("")).map(|_| {})
        } else {
            Ok(())
        }
    }
}

impl<const BOOL: usize, T> WriteUrlValue for Optional<BOOL, T>
where
    (): UsizeBool<BOOL>,
    T: WriteUrlValue,
{
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        <() as UsizeBool<BOOL>>::write(self, w)
    }
}

pub trait UsizeBool<const BOOL: usize> {
    fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Optional<BOOL, T>, D::Error>
    where
        (): UsizeBool<BOOL>;
    fn write<T: WriteUrlValue, W: UrlParamWriter>(
        x: &Optional<BOOL, T>,
        w: BufferedName<'_, W>,
    ) -> Result<(), W::E>
    where
        (): UsizeBool<BOOL>;
}

impl UsizeBool<1> for () {
    fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Optional<1, T>, D::Error> {
        T::deserialize(d).map(Optional::some)
    }
    fn write<T: WriteUrlValue, W: UrlParamWriter>(
        Optional { inner: [x] }: &Optional<1, T>,
        w: BufferedName<'_, W>,
    ) -> Result<(), W::E> {
        x.ser(w)
    }
}

impl UsizeBool<0> for () {
    fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        _: D,
    ) -> Result<Optional<0, T>, D::Error> {
        Ok(Optional::none())
    }
    fn write<T: WriteUrlValue, W: UrlParamWriter>(
        _: &Optional<0, T>,
        _: BufferedName<'_, W>,
    ) -> Result<(), W::E> {
        Ok(())
    }
}

pub struct Optional<const IS_PRESENT: usize, T>
where
    (): UsizeBool<IS_PRESENT>,
{
    inner: [T; IS_PRESENT],
}

impl<T> Optional<0, T> {
    pub fn none() -> Self {
        Self { inner: [] }
    }
    pub fn insert(&mut self, x: T) -> Optional<1, T> {
        Optional { inner: [x] }
    }
}

impl<T> Optional<1, T> {
    pub fn some(x: T) -> Self {
        Self { inner: [x] }
    }
    pub fn into_inner(self) -> T {
        let [x] = self.inner;
        x
    }
}

impl<T> Deref for Optional<1, T> {
    type Target = T;
    fn deref(&self) -> &T {
        let [x] = &self.inner;
        x
    }
}

impl<T> DerefMut for Optional<1, T> {
    fn deref_mut(&mut self) -> &mut T {
        let [x] = &mut self.inner;
        x
    }
}

impl<'de, const X: usize, T> Deserialize<'de> for Optional<X, T>
where
    (): UsizeBool<X>,
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <() as UsizeBool<X>>::deserialize(deserializer)
    }
}
