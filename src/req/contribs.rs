use wikiproc::WriteUrl;

use super::Limit;

#[derive(WriteUrl, Clone, Debug)]
#[wp(prepend_all = "uc")]
pub struct ListUserContribs {
    pub limit: Limit,
    #[wp(flatten)]
    pub selector: Selector,
    pub prop: UserContribsProp,
}

#[derive(WriteUrl, Clone, Debug)]
#[wp(mutual_exclusive)]
pub enum Selector {
    User(Vec<String>),
    UserIds(Vec<u64>),
    UserPrefix(String),
    IpRange(String),
}

wikiproc::bitflags! {
    pub struct UserContribsProp: u16 {
        const IDS           = 1 << 0;
        const TITLE         = 1 << 1;
        const TIMESTAMP     = 1 << 2;
        const COMMENT       = 1 << 3;
        const SIZE          = 1 << 4;
        const FLAGS         = 1 << 5;
        const SIZEDIFF      = 1 << 6;
        const TAGS          = 1 << 7;
        const PARSEDCOMMENT = 1 << 8;
        const ORESSCORES    = 1 << 9;
    }
}
