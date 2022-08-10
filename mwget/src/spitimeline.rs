use std::fs::{self, File};
use std::io::Write;

use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use wiki::api::QueryResponse;
use wiki::req::contribs::{ListUserContribs, Selector, UserContribsProp};
use wiki::req::events::{ListLogEvents, LogEventsProp};
use wiki::req::{Limit, Query, QueryList};
use wiki::{AccessExt, Site};

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub user: String,
    #[serde(with = "wiki::util::dt")]
    pub timestamp: DateTime<Utc>,
    pub home_wiki: &'static str,
    pub page: String,
    pub description: String,
    pub comment: String,
    pub link: String,
}

#[derive(Deserialize)]
pub struct LogEvent {
    pub logid: u64,
    pub title: String,
    #[serde(with = "wiki::util::dt")]
    pub timestamp: DateTime<Utc>,
    pub comment: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub action: String,
}

#[derive(Deserialize)]
pub struct Contrib {
    pub user: String,
    pub revid: u64,
    pub parentid: u64,
    pub minor: bool,
    pub comment: String,
    #[serde(with = "wiki::util::dt")]
    pub timestamp: DateTime<Utc>,
    pub sizediff: i64,
    pub title: String,
}

#[derive(Deserialize)]
pub struct UserContribs {
    pub usercontribs: Vec<Contrib>,
}

#[derive(Deserialize)]
pub struct LogEvents {
    pub logevents: Vec<LogEvent>,
}

pub const SITES: &[(&str, &str)] = &[
    ("arwiki", "ar.wikipedia.org"),
    ("commons", "commons.wikimedia.org"),
    ("dewiki", "de.wikipedia.org"),
    ("enwiki", "en.wikipedia.org"),
    ("fawiki", "fa.wikipedia.org"),
    ("frwiki", "fr.wikipedia.org"),
    ("idwiki", "id.wikipedia.org"),
    ("jawiki", "ja.wikipedia.org"),
    ("kowiki", "ko.wikipedia.org"),
    ("mswiki", "ms.wikipedia.org"),
    ("nlwiki", "nl.wikipedia.org"),
    ("ruwiki", "ru.wikipedia.org"),
    ("svwiki", "sv.wikipedia.org"),
    ("tawiki", "ta.wikipedia.org"),
    ("thwiki", "th.wikipedia.org"),
    ("ukwiki", "uk.wikipedia.org"),
    ("viwiki", "vi.wikipedia.org"),
    ("wd", "www.wikidata.org"),
    ("zhwiki", "zh.wikipedia.org"),
];

pub const IPS: &[&str] = &[
    "60.52.194.110",
    "115.133.45.185",
    "175.139.38.48",
    "203.106.183.173",
    "210.186.143.141",
];

pub const USERS: &[&str] = &[
    "Arrisontan",
    "Patricialiew",
    "Silva Andre Koimes",
    "Waynecai",
    "千禧一族",
    "日当たりの良い羽",
];

pub fn sort() -> crate::Result<()> {
    let s = fs::read_to_string("test.json")?;
    let mut ev: Vec<Event> = serde_json::from_str(Box::leak(s.into_boxed_str()))?;
    ev.sort_unstable_by_key(|e| e.timestamp);
    let mut f = File::create("out.txt")?;
    for Event {
        user,
        timestamp,
        home_wiki,
        page,
        description,
        comment,
        link,
    } in ev
    {
        let timestamp = timestamp.to_rfc3339();
        let wiki = &link[..link.find("/w/index.php").unwrap()];
        let pagelink = format!("{wiki}/wiki/{}", page.replace(' ', "_"));
        writeln!(f, "|-")?;
        writeln!(f, "| {user}")?;
        writeln!(f, "| [{link} {timestamp}]")?;
        writeln!(f, "| {home_wiki}")?;
        writeln!(f, "| [{pagelink} {page}]")?;
        writeln!(f, "| {description}")?;
        writeln!(f, "| <nowiki>{comment}</nowiki>")?;
    }
    Ok(())
}

pub async fn main() -> crate::Result<()> {
    let mut events = vec![];

    for (name, url) in SITES {
        let api_url = format!("https://{url}/w/api.php");
        let site = Site::new(&api_url)?;

        let q = Query {
            list: Some(
                QueryList::UserContribs(ListUserContribs {
                    selector: Selector::User(
                        USERS
                            .iter()
                            .chain(IPS)
                            .copied()
                            .map(ToOwned::to_owned)
                            .collect(),
                    ),
                    prop: UserContribsProp::COMMENT
                        | UserContribsProp::IDS
                        | UserContribsProp::SIZEDIFF
                        | UserContribsProp::TIMESTAMP
                        | UserContribsProp::TITLE
                        | UserContribsProp::FLAGS,
                    limit: Limit::Max,
                })
                .into(),
            ),
            ..Default::default()
        };

        // contribs
        site.query_all(q)
            .try_for_each(|x| {
                let ret = (|| {
                    let c: QueryResponse<UserContribs> = serde_json::from_value(x)?;
                    for contrib in c.query.usercontribs {
                        events.push(Event {
                            user: contrib.user,
                            timestamp: contrib.timestamp,
                            home_wiki: name,
                            page: contrib.title,
                            description: format!(
                                "{}{}",
                                if contrib.minor { "'''m''' " } else { "" },
                                contrib.sizediff
                            ),
                            comment: contrib.comment,
                            link: format!(
                                "https://{url}/w/index.php?diff=prev&oldid={}&diffmode=source",
                                contrib.revid
                            ),
                        })
                    }
                    Ok(())
                })();
                async { ret }
            })
            .await?;

        // logs
        for u in USERS.iter().chain(IPS) {
            let m = Query {
                list: Some(
                    QueryList::LogEvents(ListLogEvents {
                        prop: LogEventsProp::COMMENT
                            | LogEventsProp::IDS
                            | LogEventsProp::TITLE
                            | LogEventsProp::TIMESTAMP
                            | LogEventsProp::TYPE,
                        user: Some(u.to_string()),
                        limit: Limit::Max,
                    })
                    .into(),
                ),
                ..Default::default()
            };
            site.query_all(m)
                .try_for_each(|x| {
                    let ret = (|| {
                        let c: QueryResponse<LogEvents> = serde_json::from_value(x)?;
                        for LogEvent {
                            logid,
                            title,
                            timestamp,
                            comment,
                            type_,
                            action,
                        } in c.query.logevents
                        {
                            events.push(Event {
                                user: u.to_string(),
                                timestamp,
                                home_wiki: name,
                                page: title,
                                description: format!("type: {type_}, action: {action}"),
                                comment,
                                link: format!(
                                    "https://{url}/w/index.php?title=Special:Log&logid={logid}"
                                ),
                            })
                        }
                        Ok(())
                    })();
                    async { ret }
                })
                .await?;
        }
    }

    let f = File::create("test.json")?;
    serde_json::to_writer(f, &events)?;

    Ok(())
}
