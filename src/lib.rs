use std::sync::Arc;

use opendal::Operator;
use reqwest::Client;
use sqlx::SqlitePool;
use url::Url;

use crate::parser::Registry;

pub mod api;
pub mod parser;
pub mod repo;
pub mod route;
pub mod service;

mod utils;

#[derive(Clone)]
pub struct Context {
    pub db: SqlitePool,
    pub http: Client,
    pub store: Operator,
    pub config: Arc<Config>,
    pub parsers: Arc<Registry>,
}

pub struct Config {
    pub bot_api: Url,
}
