use std::num::NonZeroU32;

use wikiproc::{bitflags, WriteUrl};

use super::Limit;

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "sr")]
pub struct ListSearch {
    pub search: String,
    pub limit: Limit,
    pub prop: SearchProp,
    pub info: SearchInfo,
}

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "gsr")]
pub struct SearchGenerator {
    pub search: String,
    pub limit: Limit,
    pub offset: Option<NonZeroU32>,
    pub prop: SearchProp,
    pub info: SearchInfo,
}

bitflags! {
    pub struct SearchProp: u16 {
        const CATEGORY_SNIPPET = 1 << 0;
        const EXTENSION_DATA = 1 << 1;
        const IS_FILE_MATCH = 1 << 2;
        const REDIRECT_SNIPPET = 1 << 3;
        const SECTION_TITLE = 1 << 4;
        const SIZE = 1 << 5;
        const SNIPPET = 1 << 6;
        const TIMESTAMP = 1 << 7;
        const TITLE_SNIPPET = 1 << 8;
        const WORD_COUNT = 1 << 9;
    }
}

bitflags! {
    pub struct SearchInfo: u8 {
        const REWRITTEN_QUERY = 1 << 0;
        const SUGGESTION = 1 << 1;
        const TOTAL_HITS = 1 << 2;
    }
}
