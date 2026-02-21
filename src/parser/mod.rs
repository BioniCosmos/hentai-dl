use std::collections::HashMap;

use self::{houhuayuan::Houhuayuan, telegraph::Telegraph};

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

#[derive(Default)]
pub struct Registry {
    parsers: HashMap<&'static str, Box<dyn Parser + Send + Sync>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, parser: Box<dyn Parser + Send + Sync>) {
        self.parsers.insert(parser.domain(), parser);
    }

    pub fn get(&self, domain: &str) -> Option<&(dyn Parser + Send + Sync)> {
        self.parsers.get(domain).map(|parser| parser.as_ref())
    }
}

pub fn init_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(Telegraph));
    registry.register(Box::new(Houhuayuan));
    registry
}
