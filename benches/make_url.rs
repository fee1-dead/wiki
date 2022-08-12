use chrono::{DateTime, NaiveDateTime, Utc};
use criterion::{criterion_group, criterion_main, Criterion};
use wiki::req::{Main, EditBuilder};
use wiki::types::MwTimestamp;

fn make_url_bench(c: &mut Criterion) {
    c.benchmark_group("make_url_large_edit")
        .bench_function("parse", |bencher| {
            bencher.iter(|| {
                let u: url::Url = "https://en.wikipedia.org/w/api.php?action=edit&\
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
                formatversion=2".parse().unwrap();
                u
            });
        })
        .bench_function("builder", |bencher| {
            bencher.iter(|| {
                let main = Main::edit(
                    EditBuilder::new()
                        .title("title")
                        .token("token")
                        .bot()
                        .appendtext("app")
                        .baserevid(0)
                        .basetimestamp(MwTimestamp(DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc)))
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
                
                wiki::api::mkurl("https://en.wikipedia.org/w/api.php".parse().unwrap(), main)
            })
        });
}

criterion_group!(benches, make_url_bench);
criterion_main!(benches);
