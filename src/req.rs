use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem::discriminant;
use std::num::{NonZeroU16, NonZeroU32};

use bytemuck::TransparentWrapper;
use serde::ser::SerializeSeq;
use wikiproc::WriteUrl;

use crate::macro_support::{
    BufferedName, ApiEnum, UrlParamWriter, WriteUrlParams, WriteUrlValue, TriStr,
};
use crate::url::SerdeAdaptor;

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
            Limit::Max => { w.write(TriStr::Static("max"))?; }
            Limit::Value(v) => v.ser(w)?,
            Limit::None => {}
        }
        Ok(())
    }
}

// TODO more efficient
pub struct EnumSet<T: ApiEnum> {
    flag: T::Bitflag,
    values: Vec<T>,
}

impl<T: ApiEnum> EnumSet<T> {
    pub fn new() -> Self {
        Self {
            flag: Default::default(),
            values: Vec::new(),
        }
    }

    pub fn new_one(x: T) -> Self {
        Self { flag: x.flag(), values: vec![x] }
    }

    pub fn insert(&mut self, x: T) -> bool {
        if self.flag & x.flag() != Default::default() {
            return false;
        }
        self.flag = self.flag | x.flag();
        self.values.push(x);
        true
    }
}

impl<T: ApiEnum> Default for EnumSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait HasValue {
    const CAUTIOUS: bool;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R;
}

impl<T: ApiEnum> HasValue for T {
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

impl HasValue for u32 {
    const CAUTIOUS: bool = false;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R {
        accept(&*self.to_string())
    }
}

#[must_use]
pub fn encode_multivalue<'a, T: HasValue + 'a, V: IntoIterator<Item = &'a T> + Clone>(
    values: V,
) -> String {
    let mut sep = '|';
    let mut s = String::new();
    if T::CAUTIOUS {
        for item in values.clone() {
            if item.value(|v| v.contains('|')) {
                sep = '\u{1F}';
                s.push(sep);
                break;
            }
        }
    }
    for (i, item) in values.into_iter().enumerate() {
        if i != 0 {
            s.push(sep);
        }
        item.value(|v| {
            s.push_str(v);
        });
    }
    s
}

impl<T: ApiEnum + WriteUrlValue> WriteUrlValue for EnumSet<T> {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> crate::Result<(), W::E> {
        let s = encode_multivalue(&self.values);
        let w = w.write(s.into())?;
        self.ser_additional_only(w)
    }
    fn ser_additional_only<W: UrlParamWriter>(&self, w: &mut W) -> crate::Result<(), W::E> {
        for v in &self.values {
            v.0.ser_additional_only(w)?;
        }
        Ok(())
    }
}

impl<T: ApiEnum> FromIterator<T> for EnumSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut flag = Default::default();
        let values = iter.into_iter().inspect(|x| flag |= x.flag()).collect();
        Self { flag, values }
    }
}

impl<'a, T: ApiEnum + Clone + 'static> From<&'a [T]> for EnumSet<T> {
    fn from(x: &'a [T]) -> Self {
        x.iter().cloned().collect()
    }
}

impl<'a, T: ApiEnum> From<T> for EnumSet<T> {
    fn from(x: T) -> Self {
        Self::new_one(x)
    }
}

impl<'a, T: ApiEnum, const LEN: usize> From<[T; LEN]> for EnumSet<T> {
    fn from(arr: [T; LEN]) -> Self {
        let mut flag = Default::default();
        for x in &arr {
            flag |= x.flag();
        }
        Self { flag, values: arr.into() }
    }
}

#[derive(WriteUrl)]
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

    pub fn tokens(t: &[TokenType]) -> Self {
        Self::query(Query {
            meta: Some(QueryMeta::Tokens { type_: t.into() }.into()),
            ..Default::default()
        })
    }

    pub fn action(action: Action) -> Self {
        Self {
            action,
            format: Format::Json,
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

#[derive(WriteUrl)]
pub enum Action {
    Query(Query),
    Edit(Edit),
    Login(Login),
}

#[derive(WriteUrl, Default)]
pub struct Query {
    pub list: Option<EnumSet<QueryList>>,
    pub meta: Option<EnumSet<QueryMeta>>,
    /// Which properties to get for the queried pages.
    pub prop: Option<EnumSet<QueryProp>>,
    pub titles: Option<Vec<String>>,
    pub pageids: Option<Vec<u32>>,
    pub generator: Option<QueryGenerator>,
}

#[derive(WriteUrl)]
pub enum QueryList {
    Search(ListSearch),
    RecentChanges(ListRc),
}

#[derive(WriteUrl)]
#[wikiproc(prepend_all = "sr")]
pub struct ListSearch {
    pub search: String,
    pub limit: Limit,
    pub prop: Option<EnumSet<SearchProp>>,
}

#[derive(WriteUrl)]
pub enum SearchProp {

}

#[derive(WriteUrl)]
#[wikiproc(prepend_all = "rc")]
pub struct ListRc {
    pub limit: Limit,
    pub ty: RcType,
}

#[derive(WriteUrl)]
pub enum RcProp {
    Title,
    Timestamp,
    Ids,
    Comment,
    User
}

#[derive(WriteUrl)]
pub enum RcType {
    
}

#[derive(WriteUrl)]
pub enum QueryMeta {
    Tokens {
        #[wikiproc(name = "type")]
        type_: EnumSet<TokenType>,
    },
    UserInfo(MetaUserInfo),
}

#[derive(WriteUrl)]
#[wikiproc(prepend_all = "ui")]
pub struct MetaUserInfo {
    pub prop: EnumSet<UserInfoProp>,
}

#[derive(WriteUrl)]
pub enum UserInfoProp {
    Rights,
}

#[derive(WriteUrl)]
pub enum QueryProp {
    Revisions(QueryPropRevisions),
}

#[derive(WriteUrl)]
#[wikiproc(prepend_all = "rv")]
pub struct QueryPropRevisions {
    pub prop: EnumSet<RvProp>,
    pub slots: EnumSet<RvSlot>,
    pub limit: Limit,
}

#[derive(WriteUrl)]
pub enum QueryGenerator {
    Search(SearchGenerator),
}

#[derive(WriteUrl)]
#[wikiproc(prepend_all = "gsr")]
pub struct SearchGenerator {
    pub search: String,
    pub limit: Limit,
    pub offset: Option<NonZeroU32>,
}

#[derive(WriteUrl)]
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

#[derive(WriteUrl)]
pub enum RvSlot {
    Main,
    #[wikiproc(name = "*")]
    All,
}

#[derive(WriteUrl, Clone, Copy)]
pub enum TokenType {
    CreateAccount,
    Csrf,
    DeleteGlobalAccount,
    Login,
    Patrol,
    Rollback,
    SetGlobalAccountStatus,
    UserRights,
    Watch,
}

#[derive(WriteUrl)]
#[wikiproc(unnamed)]
pub enum PageSpec {
    Title { title: String },
    Id { pageid: u32 },
}

#[derive(WriteUrl)]
pub struct Edit {
    #[wikiproc(flatten)]
    pub spec: PageSpec,
    pub text: String,
    pub summary: String,
    pub baserevid: u32,
    pub token: String,
}

#[derive(WriteUrl)]
pub struct Login {
    #[wikiproc(name = "lgname")]
    pub name: String,
    #[wikiproc(name = "lgpassword")]
    pub password: String,
    #[wikiproc(name = "lgtoken")]
    pub token: String,
}

#[derive(WriteUrl)]
pub enum Format {
    Json,
    None,
    Php,
    RawFm,
    Xml,
}

#[derive(WriteUrl)]
#[wikiproc(prepend_all = "cm")]
pub struct ListCategoryMembers {
    #[wikiproc(flatten)]
    spec: PageSpec,
    set: Option<EnumSet<CmProps>>,
}

#[derive(WriteUrl)]
pub enum CmProps {
    Ids,
}
