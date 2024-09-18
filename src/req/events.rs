use wikiproc::WriteUrl;

use super::Limit;

#[derive(WriteUrl, Clone, Debug)]
#[wp(prepend_all = "le")]
pub struct ListLogEvents {
    pub prop: LogEventsProp,
    pub user: Option<String>,
    pub limit: Limit,
}

wikiproc::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct LogEventsProp: u16 {
        const IDS           = 1 << 0;
        const TITLE         = 1 << 1;
        const TYPE          = 1 << 2;
        const USER          = 1 << 3;
        const TIMESTAMP     = 1 << 4;
        const COMMENT       = 1 << 5;
        const DETAILS       = 1 << 6;
        const PARSEDCOMMENT = 1 << 7;
        const TAGS          = 1 << 8;
        const USERID        = 1 << 9;
        const DEFAULT = ( Self::IDS.bits()
                        | Self::TITLE.bits()
                        | Self::TYPE.bits()
                        | Self::USER.bits()
                        | Self::TIMESTAMP.bits()
                        | Self::COMMENT.bits()
                        | Self::DETAILS.bits()
                        );
    }
}
