use wikiproc::WriteUrl;

use crate::url::TriStr;
use crate::url::{WriteUrlValue, UrlParamWriter, BufferedName};
use crate::types::MwTimestamp;

#[derive(Clone)]
pub enum Expiry {
    Relative(String),
    Absolute(MwTimestamp),
    Never,
}

impl WriteUrlValue for Expiry {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        match self {
            Expiry::Absolute(timestamp) => timestamp.ser(w),
            Expiry::Relative(relative) => w.write(TriStr::Shared(relative)).map(|_| {}),
            Expiry::Never => w.write(TriStr::Static("never")).map(|_| {}),
        }
    }
}

#[derive(Clone, WriteUrl)]
pub struct Block {
    pub user: String,
    pub expiry: Expiry,
    pub reason: Option<String>,
    pub anononly: bool,
    pub nocreate: bool,
    pub autoblock: bool,
    pub noemail: bool,
    pub hidename: bool,
    pub allowusertalk: bool,
    pub reblock: bool,
    pub watchuser: bool,
    pub watchlistexpiry: Option<MwTimestamp>,
    pub tags: Option<Vec<String>>,
    pub partial: bool,
    pub pagerestrictions: Option<Vec<String>>,
    #[wp(name = "namespacerestrictions")]
    pub namespace_restrictions: Option<Vec<i32>>,
}