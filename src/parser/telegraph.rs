use anyhow::anyhow;
use scraper::{Html, Selector};
use url::Url;

use super::{ParseResult, Parser};

const DOMAIN: &str = "telegra.ph";

pub struct Telegraph;

impl Parser for Telegraph {
    fn domain(&self) -> &'static str {
        DOMAIN
    }

    fn parse(&self, raw: &str) -> anyhow::Result<ParseResult> {
        let doc = Html::parse_document(raw);
        let title = doc
            .select(&Selector::parse(".tl_article_header h1").expect("unexpected invalid selector"))
            .nth(0)
            .ok_or(anyhow!("failed to find the title"))?
            .inner_html();
        let urls = doc
            .select(&Selector::parse("img").expect("unexpected invalid selector"))
            .map(|img| img.attr("src"))
            .filter_map(|img| img.and_then(to_absolute))
            .collect();
        Ok(ParseResult::Images { title, urls })
    }
}

fn to_absolute(relative: &str) -> Option<String> {
    Url::parse(&format!("https://{DOMAIN}"))
        .unwrap()
        .join(relative)
        .ok()
        .map(|url| url.into())
}
