use wikiproc::WriteUrl;

use super::Limit;
use crate::types::NowableTime;

#[derive(Clone, Debug, WriteUrl, Default)]
#[wp(prepend_all = "abf")]
pub struct ListAbuseFilters {
    pub startid: Option<u32>,
    pub endid: Option<u32>,
    pub limit: Limit,
    pub prop: AbuseFilterProp,
}

#[derive(Clone, Debug, WriteUrl)]
#[wp(prepend_all = "afl")]
pub struct ListAbuseLog {
    pub logid: Option<u64>,
    pub start: Option<NowableTime>,
    pub end: Option<NowableTime>,
    pub filter: Option<Vec<String>>,
    pub limit: Limit,
    pub prop: AbuseLogProp,
}

wikiproc::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

wikiproc::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AbuseFilterProp: u16 {
        const ID = 1 << 0;
        const DESCRIPTION = 1 << 1;
        const ACTIONS = 1 << 2;
        const PATTERN = 1 << 3;
        const STATUS = 1 << 4;
        const PRIVATE = 1 << 5;
        const LASTEDITTIME = 1 << 6;
        const LASTEDITOR = 1 << 7;
        const HITS = 1 << 8;
        const COMMENTS = 1 << 9;
    }
}

impl Default for AbuseFilterProp {
    fn default() -> Self {
        Self::ID | Self::DESCRIPTION | Self::ACTIONS | Self::STATUS
    }
}
