use wikiproc::WriteUrl;

use super::PageSpec;

#[derive(WriteUrl, Clone, Debug, Default)]
pub struct Parse {
    pub title: Option<String>,
    pub text: Option<String>,
    #[wp(flatten)]
    pub selector: Option<PageSpec>,
    pub redirects: bool,
    pub oldid: Option<u64>,
    pub prop: ParseProp,
    pub preview: bool,
    pub pst: bool,
    pub onlypst: bool,
}

wikiproc::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ParseProp: u32 {
        const TEXT = 1 << 0;
        const LANGLINKS = 1 << 1;
        const CATEGORIES = 1 << 2;
        const CATEGORIES_HTML = 1 << 3;
        const LINKS = 1 << 4;
        const TEMPLATES = 1 << 5;
        const IMAGES = 1 << 6;
        const EXTERNAL_LINKS = 1 << 7;
        const SECTIONS = 1 << 8;
        const REV_ID = 1 << 9;
        const DISPLAY_TITLE = 1 << 10;
        const SUBTITLE = 1 << 11;
        const HEAD_HTML = 1 << 12;
        const MODULES = 1 << 13;
        const JS_CONFIG_VARS = 1 << 14;
        const ENCODED_JS_CONFIG_VARS = 1 << 15;
        const INDICATORS = 1 << 16;
        const IWLINKS = 1 << 17;
        const WIKITEXT = 1 << 18;
        const PROPERTIES = 1 << 19;
        const LIMIT_REPORT_DATA = 1 << 20;
        const LIMIT_REPORT_HTML = 1 << 21;
        const PARSE_TREE = 1 << 22;
        const PARSE_WARNINGS = 1 << 23;
        const PARSE_WARNINGS_HTML = 1 << 24;
        const DEFAULT = Self::TEXT.bits()
        | Self::LANGLINKS.bits()
        | Self::CATEGORIES.bits()
        | Self::LINKS.bits()
        | Self::TEMPLATES.bits()
        | Self::IMAGES.bits()
        | Self::EXTERNAL_LINKS.bits()
        | Self::SECTIONS.bits()
        | Self::REV_ID.bits()
        | Self::DISPLAY_TITLE.bits()
        | Self::IWLINKS.bits()
        | Self::PROPERTIES.bits()
        | Self::PARSE_WARNINGS.bits();
    }
}

impl Default for ParseProp {
    fn default() -> Self {
        Self::DEFAULT
    }
}
