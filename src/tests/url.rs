use chrono::{DateTime, NaiveDateTime, Utc};

use crate::api::mkurl;
use crate::req::{EditBuilder, Main};
use crate::types::MwTimestamp;

#[test]
fn edit() {
    let t = MwTimestamp(DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc));
    let main = Main::edit(
        EditBuilder::new()
            .title("title")
            .token("token")
            .bot()
            .appendtext("app")
            .baserevid(0)
            .basetimestamp(t)
            .captchaid("captchaid")
            .captchaword("captchaword")
            .contentformat("ctfmt")
            .contentmodel("ctmd")
            .createonly()
            .md5("md5")
            .new_section("newsection".into())
            .prependtext("prepend")
            .recreate()
            .redirect()
            .tags(vec!["a".into(), "b".into()])
            .build(),
    );
    let u = mkurl("https://en.wikipedia.org/w/api.php".parse().unwrap(), main);
    assert_eq!("https://en.wikipedia.org/w/api.php?action=edit&\
    title=title&\
    section=new&\
    sectiontitle=newsection&\
    tags=a%7Cb&\
    bot=&\
    baserevid=0&\
    basetimestamp=1970-01-01T00%3A00%3A00Z&\
    recreate=&\
    createonly=&\
    md5=md5&\
    prependtext=prepend&\
    appendtext=app&\
    redirect=&\
    contentformat=ctfmt&\
    contentmodel=ctmd&\
    token=token&\
    captchaword=captchaword&\
    captchaid=captchaid&\
    format=json&\
    formatversion=2", u.to_string())
}
