pub mod houhuayuan;
pub mod telegraph;

pub trait Parser {
    fn domain(&self) -> &'static str;
    fn parse(&self, raw: &str) -> anyhow::Result<ParseResult>;
}

#[derive(Debug)]
pub enum ParseResult {
    Markdown { title: String, body: String },
    Images { title: String, urls: Vec<String> },
}
