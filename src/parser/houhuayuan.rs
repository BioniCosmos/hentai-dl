use anyhow::Context;
use scraper::{Html, Selector};

use super::{ParseResult, Parser};

pub struct Houhuayuan;

impl Parser for Houhuayuan {
    fn domain(&self) -> &'static str {
        "houhuayuan.vip"
    }

    fn parse(&self, raw: &str) -> anyhow::Result<ParseResult> {
        let doc = Html::parse_document(raw);
        let title = doc
            .select(&Selector::parse("title").expect("unexpected invalid selector"))
            .next()
            .context("failed to parse the title")?
            .inner_html()
            .split(" – ")
            .next()
            .context("failed to parse the title")?
            .to_owned();
        let body = doc
            .select(&Selector::parse(".entry-content").expect("unexpected invalid selector"))
            .next()
            .context("failed to parse the body")?
            .select(&Selector::parse(":scope > p").expect("unexpected invalid selector"))
            .map(|p| p.inner_html())
            .collect::<Vec<_>>()
            .join("\n\n");
        let body = format!("# {title}\n\n{body}\n");
        Ok(ParseResult::Markdown { title, body })
    }
}
