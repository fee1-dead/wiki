use wikiproc::WriteUrl;

use super::Limit;
use crate::types::NowableTime;

#[derive(Clone, Debug, WriteUrl)]
#[wp(prepend_all = "afl")]
pub struct ListAbuseLog {
    pub start: Option<NowableTime>,
    pub end: Option<NowableTime>,
    pub filter: Option<Vec<String>>,
    pub limit: Limit,
    pub prop: AbuseLogProp,
}

wikiproc::bitflags! {
    pub struct AbuseLogProp: u16 {
        const DETAILS   = 1 << 0;
        const ACTION    = 1 << 1;
        const FILTER    = 1 << 2;
        const HIDDEN    = 1 << 3;
        const IDS       = 1 << 4;
        const RESULT    = 1 << 5;
        const REVID     = 1 << 6;
        const TIMESTAMP = 1 << 7;
        const TITLE     = 1 << 8;
        const USER      = 1 << 9;
    }
}
