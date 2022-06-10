use std::{collections::HashSet, fmt::Debug, hash::Hash, mem::discriminant};

use bytemuck::TransparentWrapper;
use serde::ser::SerializeSeq;
use wikiproc::WriteUrl;

use crate::{NamedEnum, WriteUrlValue, TriStr, WriteUrlParams, SerdeAdaptor};

#[derive(TransparentWrapper)]
#[repr(transparent)]
pub struct SerializeAdaptor<T>(pub T);

impl<T: WriteUrlParams> serde::Serialize for SerializeAdaptor<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
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

pub struct EnumSet<T> {
    set: HashSet<VariantBased<T>>,
}

impl<T> EnumSet<T> {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }

    pub fn new_one(x: T) -> Self {
        let mut this = Self::new();
        this.insert(x);
        this
    }

    pub fn insert(&mut self, x: T) -> bool {
        self.set.insert(VariantBased(x))
    }
}

pub trait HasValue {
    const CAUTIOUS: bool;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R;
}

impl<T: NamedEnum> HasValue for VariantBased<T> {
    const CAUTIOUS: bool = false;
    fn value<F: FnOnce(&str) -> R, R>(&self, accept: F) -> R {
        (accept)(self.0.variant_name())
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

impl<T: NamedEnum + WriteUrlValue> WriteUrlValue for EnumSet<T> {
    fn ser<W: crate::UrlParamWriter>(
        &self,
        w: crate::BufferedName<'_, W>,
    ) -> crate::Result<(), W::E> {
        if self.set.is_empty() {
            return Ok(());
        }
        let s = encode_multivalue(&self.set);
        let w = w.write(TriStr::Owned(s))?;
        for v in &self.set {
            v.0.ser_additional_only(w)?;
        }
        Ok(())
    }
}

impl<T> FromIterator<T> for EnumSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let set = iter.into_iter().map(VariantBased).collect();
        Self { set }
    }
}

impl<'a, T: Clone + 'static> FromIterator<&'a T> for EnumSet<T> {
    fn from_iter<I: IntoIterator<Item = &'a T>>(iter: I) -> Self {
        let set = iter.into_iter().cloned().map(VariantBased).collect();
        Self { set }
    }
}

impl<'a, T: Clone + 'static> From<&'a [T]> for EnumSet<T> {
    fn from(x: &'a [T]) -> Self {
        x.iter().collect()
    }
}

impl<'a, T> From<T> for EnumSet<T> {
    fn from(x: T) -> Self {
        Self::new_one(x)
    }
}

impl<'a, T> From<[T; 0]> for EnumSet<T> {
    fn from([]: [T; 0]) -> Self {
        Self::new()
    }
}

#[derive(WriteUrl)]
pub struct Main {
    pub action: Action,
    pub format: Format,
}

impl Main {
    pub fn tokens(t: &[TokenType]) -> Self {
        Self {
            action: Action::Query(Query {
                list: [].into(),
                meta: QueryMeta::Tokens {
                    type_: t.into(),
                }.into(),
            }),
            format: Format::Json,
        }
    }

    pub fn build_form(&self) -> reqwest::multipart::Form {
        let mut f = reqwest::multipart::Form::new();
        if let Err(inf) = self.ser(&mut f) {
            match inf {}
        }
        f
    }
}

#[derive(WriteUrl)]
pub enum Action {
    Query(Query),
    Edit(Edit),
}

#[derive(WriteUrl)]
pub struct Query {
    pub list: EnumSet<QueryList>,
    pub meta: EnumSet<QueryMeta>,
}

#[derive(WriteUrl)]
pub enum QueryList {
    Search { srsearch: String },
}

#[derive(WriteUrl)]
#[non_exhaustive]
pub enum QueryMeta {
    Tokens {
        #[wikiproc(name = "type")]
        type_: EnumSet<TokenType>,
    },
}

#[derive(WriteUrl, Clone, Copy)]
#[non_exhaustive]
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
    Id { id: u32 },
}

#[derive(WriteUrl)]
pub struct Edit {
    #[wikiproc(flatten)]
    pub spec: PageSpec,
}

#[derive(WriteUrl)]
pub enum Format {
    Json,
    None,
    Php,
    RawFm,
    Xml,
}
