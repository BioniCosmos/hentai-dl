use anyhow::anyhow;
use scraper::{Html, Selector};

use super::{ParseResult, Parser};

pub struct Telegraph;

impl Parser for Telegraph {
    fn domain(&self) -> &'static str {
        "telegra.ph"
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
            .filter(|img| img.is_some())
            .map(|img| img.unwrap().to_owned())
            .collect();
        Ok(ParseResult::Images { title, urls })
    }
}
