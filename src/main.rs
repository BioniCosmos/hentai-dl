use std::sync::Arc;

use axum::Router;
use opendal::Operator;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

use hentai_dl::{
    Config, Context, api, parser,
    route::{self},
    service::bot as bot_svc,
};

#[tokio::main]
async fn main() {
    init_trace();

    let database_url = dotenvy::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_owned());
    let storage_url = dotenvy::var("STORAGE_URL").unwrap_or("fs://?root=.".to_owned());
    let bot_token = dotenvy::var("BOT_TOKEN").expect("expecting `BOT_TOKEN`");
    let base_url = dotenvy::var("BASE_URL").expect("expecting `BASE_URL`");

    let ctx = Context {
        db: SqlitePool::connect(&database_url)
            .await
            .expect("failed to connect to database"),
        http: Client::new(),
        store: init_operator(&storage_url),
        config: Arc::new(Config {
            bot_api: api::init(&bot_token),
        }),
        parsers: Arc::new(parser::init_registry()),
    };

    bot_svc::set_webhook(&ctx, &base_url)
        .await
        .expect("failed to initialize bot service");
    start(ctx).await;
}

fn init_trace() {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    tracing_subscriber::registry()
        .with(fmt::layer().pretty())
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("hentai_dl=trace,tower_http=trace,axum=trace"))
                .expect("failed to initialize the filter layer"),
        )
        .init();
}

fn init_operator(storage_url: &str) -> Operator {
    Operator::from_uri(storage_url).expect("failed to initialize storage operator")
}

async fn start(state: Context) {
    let app = route::register_bot_webhook(route::register(Router::new()))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind the listener");
    info!(
        "listening on http://{}",
        listener
            .local_addr()
            .expect("failed to get the listening address")
    );

    axum::serve(listener, app)
        .await
        .expect("failed to start the web service");
}
