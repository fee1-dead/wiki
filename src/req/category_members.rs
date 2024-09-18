use serde::Deserialize;
use wikiproc::{bitflags, WriteUrl};

use super::{Limit, PageSpec};
use crate::build_response_type;

#[derive(WriteUrl, Clone, Debug)]
#[wp(prepend_all = "cm")]
pub struct ListCategoryMembers {
    #[wp(flatten)]
    pub spec: PageSpec,
    pub limit: Limit,
    #[wp(name = "type")]
    pub ty: CategoryMembersType,
    pub prop: CategoryMembersProp,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CategoryMembersType: u8 {
        const FILE = 1;
        const PAGE = 2;
        const SUBCAT = 4;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CategoryMembersProp: u8 {
        const IDS = 1 << 0;
        const SORT_KEY = 1 << 1;
        const SORT_KEY_PREFIX = 1 << 2;
        const TIMESTAMP = 1 << 3;
        const TITLE = 1 << 4;
        const TYPE = 1 << 5;
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CategoryMember {
    pub pageid: Option<u32>,
    pub ns: Option<i64>,
    pub title: Option<String>,
    pub sortkey: Option<String>,
    pub sortkeyprefix: Option<String>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub timestamp: Option<String>,
}

build_response_type! {
    #[derive(Clone)]
    CategoryMembersResponse { categorymembers: Vec<CategoryMember> }
}
