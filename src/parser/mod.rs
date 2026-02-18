pub trait Parser {
    fn domain(&self) -> &'static str;
    fn parse(&self, raw: &str) -> Result<ParseResult, Error>;
}

pub enum ParseResult {
    Markdown,
    Images,
}

pub struct Error;
