use std::collections::HashSet;
use std::pin::Pin;

use futures_util::{Future, TryStreamExt};
use serde::Deserialize;
use tracing_subscriber::EnvFilter;
use wiki::api::{QueryResponse, RequestBuilderExt};
use wiki::ClientBuilder;
use wiki::events::{OldNew, RecentChangeEvent};
use wiki::req::category_members::{
    CategoryMember, CategoryMembersProp, CategoryMembersResponse, CategoryMembersType,
    ListCategoryMembers,
};
use wiki::req::parse::{Parse as RParse, ParseProp};
use wiki::req::{Action, EditBuilder, Limit, PageSpec, Query, QueryList};
use wiki::Bot;

#[derive(Deserialize, Debug)]
pub struct Link {
    pub exists: bool,
    pub ns: i64,
    pub title: String,
}

#[derive(Deserialize, Debug)]
pub struct Parse {
    pub links: Vec<Link>,
}

#[derive(Deserialize, Debug)]
pub struct Response {
    pub parse: Parse,
}

fn handle_outer<'a>(
    bot: &'a Bot,
    res: QueryResponse<CategoryMembersResponse>,
    pages: &'a mut HashSet<String>,
) -> Pin<Box<dyn Future<Output = wiki::Result<()>> + 'a>> {
    Box::pin(handle(bot, res, pages))
}

async fn handle(
    bot: &Bot,
    res: QueryResponse<CategoryMembersResponse>,
    pages: &mut HashSet<String>,
) -> wiki::Result<()> {
    for member in res.query.categorymembers {
        match member {
            CategoryMember {
                ns: Some(0),
                title: Some(title),
                ty: Some(ty),
                ..
            } if ty == "page" => {
                pages.insert(title);
            }
            CategoryMember {
                pageid: Some(pageid),
                ty: Some(ty),
                ..
            } if ty == "subcat" => {
                let res = bot
                    .get(Action::Query(Query {
                        list: Some(
                            QueryList::CategoryMembers(ListCategoryMembers {
                                spec: PageSpec::PageId(pageid),
                                ty: CategoryMembersType::SUBCAT | CategoryMembersType::PAGE,
                                prop: CategoryMembersProp::IDS
                                    | CategoryMembersProp::TYPE
                                    | CategoryMembersProp::TITLE,
                                limit: Limit::Max,
                            })
                            .into(),
                        ),
                        ..Default::default()
                    }))
                    .send_parse()
                    .await?;
                handle_outer(bot, res, pages).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> wiki::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let stream = wiki::events::ReqwestSseStream::recent_changes().await?;
    let bot = ClientBuilder::enwiki()
        .oauth(include_str!("../../bot_oauth.txt.secret"))
        .build()
        .await?;
    let botr = &bot;
    let mut pages = HashSet::new();
    let res = bot
        .get(Action::Query(Query {
            list: Some(
                QueryList::CategoryMembers(ListCategoryMembers {
                    spec: PageSpec::Title("Category:Pornographic film actors".into()),
                    ty: CategoryMembersType::SUBCAT | CategoryMembersType::PAGE,
                    prop: CategoryMembersProp::IDS
                        | CategoryMembersProp::TYPE
                        | CategoryMembersProp::TITLE,
                    limit: Limit::Max,
                })
                .into(),
            ),
            ..Default::default()
        }))
        .send_parse()
        .await?;
    handle(botr, res, &mut pages).await?;
    let bad_pages = &pages;
    stream
        .try_for_each_concurrent(None, |x| async move {
            match x {
                RecentChangeEvent {
                    revision:
                        Some(OldNew {
                            old: Some(old),
                            new: Some(new),
                        }),
                    wiki: Some(wiki),
                    namespace: Some(0),
                    ..
                } if wiki == "enwiki" => {
                    let res: Response = botr
                        .get(Action::Parse(RParse {
                            oldid: Some(old),
                            prop: ParseProp::LINKS,
                            ..Default::default()
                        })).send_parse().await?;
                    let res2: Response = botr
                        .get(Action::Parse(RParse {
                            oldid: Some(new),
                            prop: ParseProp::LINKS,
                            ..Default::default()
                        })).send_parse().await?;

                    let prev_links: HashSet<_> = res
                        .parse
                        .links
                        .into_iter()
                        .filter(|l| l.ns == 0)
                        .map(|l| l.title)
                        .collect();
                    for Link { title, ns, .. } in res2.parse.links {
                        if prev_links.contains(&title) || ns != 0 || !bad_pages.contains(&title) {
                            continue;
                        }
                        let token = botr.get_csrf_token().await?;
                        let edit = EditBuilder::new()
                            .title("title")
                            .summary("Loggin action")
                            .appendtext(format!("*https://en.wikipedia.org/w/index.php?oldid={old}&diff={new}&diffmode=source\n")).token(token.token).bot().build();
                        botr.post(Action::Edit(edit)).send_and_report_err().await?;
                    }
                }
                _ => {}
            }
            Ok(())
        })
        .await?;
    Ok(())
}
