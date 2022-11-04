use std::fmt::Debug;
use std::hash::Hash;
use std::mem::discriminant;
use std::num::NonZeroU32;

use bytemuck::TransparentWrapper;
use serde::ser::SerializeSeq;
use wikiproc::WriteUrl;

use crate::macro_support::{
    BufferedName, NamedEnum, TriStr, UrlParamWriter, WriteUrlParams, WriteUrlValue,
};
use crate::types::MwTimestamp;
use crate::url::{BitflaggedEnum, SerdeAdaptor};

pub mod abuse_log;
pub mod block;
pub mod category_members;
pub mod contribs;
pub mod events;
pub mod parse;

#[derive(TransparentWrapper)]
#[repr(transparent)]
pub struct SerializeAdaptor<T>(pub T);

impl<T: WriteUrlParams> serde::Serialize for SerializeAdaptor<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut w = SerdeAdaptor(serializer.serialize_seq(None)?);
        self.0.ser(&mut w)?;
        w.0.end()
    }
}

#[derive(bytemuck::TransparentWrapper, Clone, Copy)]
#[repr(transparent)]
pub struct VariantBased<T>(pub T);

impl<T> Hash for VariantBased<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        discriminant(&self.0).hash(state)
    }
}

impl<T> PartialEq for VariantBased<T> {
    fn eq(&self, other: &Self) -> bool {
        discriminant(&self.0).eq(&discriminant(&other.0))
    }
}

impl<T> Eq for VariantBased<T> {}

impl<T: Debug> Debug for VariantBased<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Limit {
    Max,
    Value(usize),
    None,
}

impl WriteUrlValue for Limit {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        match self {
            Limit::Max => {
                w.write(TriStr::Static("max"))?;
            }
            Limit::Value(v) => v.ser(w)?,
            Limit::None => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditSection {
    Num(u32),
    New { title: String },
    Custom(String),
}

impl WriteUrlValue for EditSection {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        match self {
            EditSection::Custom(s) => w.write(TriStr::Shared(s)).map(|_| {}),
            EditSection::Num(n) => w.write(format!("{n}").into()).map(|_| {}),
            EditSection::New { title } => w
                .write(TriStr::Shared("new"))?
                .add(TriStr::Static("sectiontitle"), TriStr::Shared(title)),
        }
    }
    fn ser_additional_only<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E> {
        match self {
            EditSection::New { title } => {
                w.add(TriStr::Static("sectiontitle"), TriStr::Shared(title))
            }
            _ => Ok(()),
        }
    }
}

// TODO more efficient
#[derive(Clone)]
pub struct EnumSet<T: BitflaggedEnum> {
    flag: T::Bitflag,
    values: Vec<T>,
}

impl<T: BitflaggedEnum> EnumSet<T> {
    pub fn new() -> Self {
        Self {
            flag: Default::default(),
            values: Vec::new(),
        }
    }

    pub fn new_one(x: T) -> Self {
        Self {
            flag: x.flag(),
            values: vec![x],
        }
    }

    pub fn insert(&mut self, x: T) -> bool {
        if self.flag & x.flag() != Default::default() {
            return false;
        }
        self.flag |= x.flag();
        self.values.push(x);
        true
    }
}

impl<T: BitflaggedEnum> Default for EnumSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait HasValue {
    const CAUTIOUS: bool;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R;
}

impl<T: NamedEnum> HasValue for T {
    const CAUTIOUS: bool = false;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R {
        accept(self.variant_name())
    }
}

impl HasValue for String {
    const CAUTIOUS: bool = true;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R {
        accept(self)
    }
}

macro_rules! has_value_to_string {
    ($($ty:ty,)*) => {$(
        impl HasValue for $ty {
            const CAUTIOUS: bool = false;
            fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R {
                accept(&*self.to_string())
            }
        }
    )*};
}

has_value_to_string! {
    i32,
    u32,
    u64,
}

pub struct MultiValueEncoder {
    s: String,
    sep: char,
    empty: bool,
}

impl MultiValueEncoder {
    pub fn new(use_unicode_separator: bool) -> Self {
        Self {
            s: if use_unicode_separator {
                '\u{1F}'.into()
            } else {
                String::new()
            },
            sep: if use_unicode_separator { '\u{1F}' } else { '|' },
            empty: true,
        }
    }

    pub fn push(&mut self, s: &str) {
        if self.empty {
            self.empty = false;
        } else {
            self.s.push(self.sep);
        }

        self.s.push_str(s);
    }

    pub fn build(self) -> String {
        self.s
    }
}

#[must_use]
pub fn encode_multivalue<'a, T: HasValue + 'a, V: IntoIterator<Item = &'a T> + Clone>(
    values: V,
) -> String {
    let use_unicode = T::CAUTIOUS
        && values
            .clone()
            .into_iter()
            .any(|i| i.value(|v| v.contains('|')));
    let mut encoder = MultiValueEncoder::new(use_unicode);
    values
        .into_iter()
        .for_each(|s| s.value(|s| encoder.push(s)));
    encoder.build()
}

impl<T: BitflaggedEnum + NamedEnum + WriteUrlValue> WriteUrlValue for EnumSet<T> {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> crate::Result<(), W::E> {
        let s = encode_multivalue(&self.values);
        let w = w.write(s.into())?;
        self.ser_additional_only(w)
    }
    fn ser_additional_only<W: UrlParamWriter>(&self, w: &mut W) -> crate::Result<(), W::E> {
        for v in &self.values {
            v.ser_additional_only(w)?;
        }
        Ok(())
    }
}

impl<T: BitflaggedEnum> FromIterator<T> for EnumSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut flag = Default::default();
        let values = iter.into_iter().inspect(|x| flag |= x.flag()).collect();
        Self { flag, values }
    }
}

impl<'a, T: BitflaggedEnum + Clone + 'static> From<&'a [T]> for EnumSet<T> {
    fn from(x: &'a [T]) -> Self {
        x.iter().cloned().collect()
    }
}

impl<'a, T: BitflaggedEnum> From<T> for EnumSet<T> {
    fn from(x: T) -> Self {
        Self::new_one(x)
    }
}

impl<'a, T: BitflaggedEnum, const LEN: usize> From<[T; LEN]> for EnumSet<T> {
    fn from(arr: [T; LEN]) -> Self {
        let mut flag = Default::default();
        for x in &arr {
            flag |= x.flag();
        }
        Self {
            flag,
            values: arr.into(),
        }
    }
}

#[derive(WriteUrl, Clone)]
pub struct Main {
    pub action: Action,
    pub format: Format,
}

impl Main {
    pub fn build_form(&self) -> reqwest::multipart::Form {
        let mut f = reqwest::multipart::Form::new();
        if let Err(inf) = self.ser(&mut f) {
            match inf {}
        }
        f
    }

    pub fn tokens(t: TokenType) -> Self {
        Self::query(Query {
            meta: Some(QueryMeta::Tokens { type_: t }.into()),
            ..Default::default()
        })
    }

    pub fn action(action: Action) -> Self {
        Self {
            action,
            format: Format::Json { formatversion: 2 },
        }
    }

    pub fn query(q: Query) -> Self {
        Self::action(Action::Query(q))
    }

    pub fn login(l: Login) -> Self {
        Self::action(Action::Login(l))
    }

    pub fn edit(e: Edit) -> Self {
        Self::action(Action::Edit(e))
    }
}

#[derive(WriteUrl, Clone)]
pub enum Action {
    Query(Query),
    Edit(Edit),
    Login(Login),
    Parse(parse::Parse),
    Block(block::Block),
}

#[derive(WriteUrl, Default, Clone)]
pub struct Query {
    pub list: Option<EnumSet<QueryList>>,
    pub meta: Option<EnumSet<QueryMeta>>,
    /// Which properties to get for the queried pages.
    pub prop: Option<EnumSet<QueryProp>>,
    pub titles: Option<Vec<String>>,
    pub pageids: Option<Vec<u32>>,
    pub generator: Option<QueryGenerator>,
}

#[derive(WriteUrl, Clone)]
pub enum QueryList {
    Search(ListSearch),
    RecentChanges(rc::ListRc),
    AbuseLog(abuse_log::ListAbuseLog),
    LogEvents(events::ListLogEvents),
    UserContribs(contribs::ListUserContribs),
    CategoryMembers(category_members::ListCategoryMembers),
}

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "sr")]
pub struct ListSearch {
    pub search: String,
    pub limit: Limit,
}

pub mod rc;

#[derive(WriteUrl, Clone)]
pub enum QueryMeta {
    Tokens {
        #[wp(name = "type")]
        type_: TokenType,
    },
    UserInfo(MetaUserInfo),
}

// TODO rewrite
#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "ui")]
pub struct MetaUserInfo {
    pub prop: Option<EnumSet<UserInfoProp>>,
}

#[derive(WriteUrl, Clone)]
pub enum UserInfoProp {
    Rights,
}

#[derive(WriteUrl, Clone)]
pub enum QueryProp {
    Revisions(QueryPropRevisions),
}

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "rv")]
pub struct QueryPropRevisions {
    pub prop: EnumSet<RvProp>,
    pub slots: EnumSet<RvSlot>,
    pub limit: Limit,
}

#[derive(WriteUrl, Clone)]
pub enum QueryGenerator {
    Search(SearchGenerator),
}

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "gsr")]
pub struct SearchGenerator {
    pub search: String,
    pub limit: Limit,
    pub offset: Option<NonZeroU32>,
}

#[derive(WriteUrl, Clone)]
pub enum RvProp {
    Comment,
    Content,
    ContentModel,
    Flagged,
    Flags,
    Ids,
    OresScores,
    ParsedComment,
    Roles,
    Sha1,
    Size,
    SlotSha1,
    SlotSize,
    Tags,
    Timestamp,
    User,
    UserId,
}

#[derive(WriteUrl, Clone)]
pub enum RvSlot {
    Main,
    #[wp(name = "*")]
    All,
}

wikiproc::bitflags! {
    pub struct TokenType: u16 {
        const CREATE_ACCOUNT = 1 << 0;
        const CSRF = 1 << 1;
        const DELETE_GLOBAL_ACCOUNT = 1 << 2;
        const LOGIN = 1 << 3;
        const PATROL = 1 << 4;
        const ROLLBACK = 1 << 5;
        const SET_GLOBAL_ACCOUNT_STATUS = 1 << 6;
        const USER_RIGHTS = 1 << 7;
        const WATCH = 1 << 8;
    }
}

#[derive(WriteUrl, Clone, Debug)]
#[wp(mutual_exclusive)]
pub enum PageSpec {
    Title(String),
    PageId(u32),
}

#[derive(WriteUrl, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watchlist {
    NoChange,
    Preferences,
    Unwatch,
    Watch,
}

#[derive(WriteUrl, Clone)]
pub struct Edit {
    #[wp(flatten)]
    pub spec: PageSpec,
    pub section: Option<EditSection>,
    pub text: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
    pub minor: bool,
    pub notminor: bool,
    pub bot: bool,
    pub baserevid: Option<u32>,
    pub basetimestamp: Option<MwTimestamp>,
    pub starttimestamp: Option<MwTimestamp>,
    pub recreate: bool,
    pub createonly: bool,
    pub nocreate: bool,
    pub watchlist: Option<Watchlist>,
    pub watchlistexpiry: Option<MwTimestamp>,
    pub md5: Option<String>,
    pub prependtext: Option<String>,
    pub appendtext: Option<String>,
    pub undo: Option<u32>,
    pub undoafter: Option<u32>,
    pub redirect: bool,
    pub contentformat: Option<String>,
    pub contentmodel: Option<String>,
    pub token: String,
    pub captchaword: Option<String>,
    pub captchaid: Option<String>,
}

#[derive(Clone, Default)]
pub struct EditBuilder {
    spec: Option<PageSpec>,
    section: Option<EditSection>,
    text: Option<String>,
    summary: Option<String>,
    tags: Option<Vec<String>>,
    minor: bool,
    notminor: bool,
    bot: bool,
    baserevid: Option<u32>,
    basetimestamp: Option<MwTimestamp>,
    starttimestamp: Option<MwTimestamp>,
    recreate: bool,
    createonly: bool,
    nocreate: bool,
    watchlist: Option<Watchlist>,
    watchlistexpiry: Option<MwTimestamp>,
    md5: Option<String>,
    prependtext: Option<String>,
    appendtext: Option<String>,
    undo: Option<u32>,
    undoafter: Option<u32>,
    redirect: bool,
    contentformat: Option<String>,
    contentmodel: Option<String>,
    token: Option<String>,
    captchaword: Option<String>,
    captchaid: Option<String>,
}

macro_rules! builder_fns {
    (@($name:ident : bool)) => {
        pub fn $name(mut self) -> Self {
            self.$name = true;
            self
        }
    };

    (@($name:ident : Option<String>)) => {
        pub fn $name(mut self, value: impl Into<String>) -> Self {
            self.$name = Some(value.into());
            self
        }
    };

    (@($name:ident : Option<$ty:path>)) => {
        pub fn $name(mut self, value: $ty) -> Self {
            self.$name = Some(value);
            self
        }
    };

    ($($name:ident : $ident: ident $( <$gen:ident $( <$gen2:ident>>)? $(>)? )?),*,) => {
        // this trick is done to avoid rustc from wrapping a $:path or $:ty which means that we cannot match on them.
        $(
            builder_fns! {
                @($name: $ident $( <$gen $(<$gen2>)?> )?)
            }
        )*
    };
}

impl EditBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Edit {
        self.try_build().expect("expected page spec and token")
    }

    pub fn try_build(self) -> Option<Edit> {
        match self {
            EditBuilder {
                spec: Some(spec),
                section,
                text,
                summary,
                tags,
                minor,
                notminor,
                bot,
                baserevid,
                basetimestamp,
                starttimestamp,
                recreate,
                createonly,
                nocreate,
                watchlist,
                watchlistexpiry,
                md5,
                prependtext,
                appendtext,
                undo,
                undoafter,
                redirect,
                contentformat,
                contentmodel,
                token: Some(token),
                captchaword,
                captchaid,
            } => Some(Edit {
                spec,
                section,
                text,
                summary,
                tags,
                minor,
                notminor,
                bot,
                baserevid,
                basetimestamp,
                starttimestamp,
                recreate,
                createonly,
                nocreate,
                watchlist,
                watchlistexpiry,
                md5,
                prependtext,
                appendtext,
                undo,
                undoafter,
                redirect,
                contentformat,
                contentmodel,
                token,
                captchaword,
                captchaid,
            }),
            _ => None,
        }
    }

    pub fn page_id(mut self, id: u32) -> Self {
        self.spec = Some(PageSpec::PageId(id));
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.spec = Some(PageSpec::Title(title.into()));
        self
    }

    pub fn new_section(mut self, title: String) -> Self {
        self.section = Some(EditSection::New { title });
        self
    }

    pub fn section_id(mut self, id: u32) -> Self {
        self.section = Some(EditSection::Num(id));
        self
    }

    pub fn section_custom(mut self, custom: String) -> Self {
        self.section = Some(EditSection::Custom(custom));
        self
    }

    builder_fns! {
        text: Option<String>,
        summary: Option<String>,
        tags: Option<Vec<String>>,
        minor: bool,
        notminor: bool,
        bot: bool,
        baserevid: Option<u32>,
        basetimestamp: Option<MwTimestamp>,
        starttimestamp: Option<MwTimestamp>,
        recreate: bool,
        createonly: bool,
        nocreate: bool,
        watchlist: Option<Watchlist>,
        watchlistexpiry: Option<MwTimestamp>,
        md5: Option<String>,
        prependtext: Option<String>,
        appendtext: Option<String>,
        undo: Option<u32>,
        undoafter: Option<u32>,
        redirect: bool,
        contentformat: Option<String>,
        contentmodel: Option<String>,
        token: Option<String>,
        captchaword: Option<String>,
        captchaid: Option<String>,
    }
}

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "lg")]
pub struct Login {
    pub name: String,
    pub password: String,
    pub token: String,
}

#[derive(WriteUrl, Clone, Copy)]
pub enum Format {
    Json { formatversion: u8 },
    None,
    Php,
    RawFm,
    Xml,
}
