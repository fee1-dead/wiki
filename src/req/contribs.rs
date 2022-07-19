use wikiproc::WriteUrl;

use super::Limit;

#[derive(WriteUrl, Clone, Debug)]
#[wp(prepend_all = "uc")]
pub struct ListUserContribs {
    pub limit: Limit,
    #[wp(flatten)]
    pub selector: Selector,
}

#[derive(WriteUrl, Clone, Debug)]
#[wp(mutual_exclusive)]
pub enum Selector {
    User(Vec<String>),
    UserIds(Vec<u64>),
    UserPrefix(String),
}