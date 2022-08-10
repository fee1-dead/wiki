use wikiproc::WriteUrl;

use super::Limit;
use crate::types::NowableTime;

#[derive(WriteUrl, Clone)]
#[wp(prepend_all = "rc")]
pub struct ListRc {
    pub start: Option<NowableTime>,
    pub end: Option<NowableTime>,
    pub limit: Limit,
    pub prop: RcProp,
    pub ty: RcType,
}

#[rustfmt::skip]
wikiproc::bitflags! {
    pub struct RcProp: u16 {
        const TITLE          = 1 <<  0;
        const TIMESTAMP      = 1 <<  1;
        const IDS            = 1 <<  2;
        const FLAGS          = 1 <<  3;
        const LOG_INFO       = 1 <<  4;
        const ORES_SCORES    = 1 <<  5;
        const PARSED_COMMENT = 1 <<  6;
        const PATROLLED      = 1 <<  7;
        const REDIRECT       = 1 <<  8;
        const SHA1           = 1 <<  9;
        const SIZES          = 1 << 10;
        const TAGS           = 1 << 11;
    }
}

#[rustfmt::skip]
wikiproc::bitflags! {
    pub struct RcType: u8 {
        const EDIT       = 1 << 0;
        const NEW        = 1 << 1;
        const EXTERNAL   = 1 << 2;
        const LOG        = 1 << 3;
        const CATEGORIZE = 1 << 4;
    }
}
