use wikiproc::WriteUrl;

#[derive(WriteUrl, Clone)]
pub struct CheckMatch {
    pub filter: String,
    #[wp(flatten)]
    pub test: CheckMatchTest,
}

#[derive(WriteUrl, Clone)]
#[wp(mutual_exclusive)]
pub enum CheckMatchTest {
    Vars(String),
    RcId(u64),
    LogId(u64),
}