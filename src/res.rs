use serde::Deserialize;

#[derive(Deserialize)]
pub struct QueryResponse<PageExt> {
    pub pages: Option<Vec<PageResponse<PageExt>>>,
}

#[derive(Deserialize)]
pub struct PageResponse<Ext> {
    #[serde(rename = "pageid")]
    pub page_id: u64,
    pub ns: i64,
    pub title: String,
    #[serde(flatten)]
    pub ext: Ext,
}
