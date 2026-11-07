use opendal::Operator;
use reqwest::Client;
use sqlx::SqlitePool;
use url::Url;

pub mod api;
pub mod parser;
pub mod repo;
pub mod route;
pub mod service;

mod utils;

struct Context {
    db: SqlitePool,
    http: Client,
    store: Operator,
    config: Config,
}

struct Config {
    bot_api: Url,
}
