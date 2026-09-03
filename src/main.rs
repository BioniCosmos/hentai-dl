use std::sync::Arc;

use axum::Router;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

use hentai_dl::{
    api::BotAPI,
    parser,
    repo::{ConfigRepo, TaskRepo},
    route::{self, AppState},
    service::{BotSvc, DownloadSvc},
};

#[tokio::main]
async fn main() {
    init_trace();

    let database_url = dotenvy::var("DATABASE_URL").ok();
    let bot_token = dotenvy::var("BOT_TOKEN").expect("expecting `BOT_TOKEN`");
    let base_url = dotenvy::var("BASE_URL").expect("expecting `BASE_URL`");

    let db = SqlitePool::connect(database_url.as_deref().unwrap_or("sqlite::memory:"))
        .await
        .expect("failed to connect to database");
    let task_repo = TaskRepo::new(db.clone());
    let config_repo = ConfigRepo::new(db);

    let parser_registry = Arc::new(parser::init_registry());
    let download_svc = DownloadSvc::new(parser_registry, task_repo);

    let bot_api = BotAPI::new(&bot_token);
    let bot_svc = BotSvc::new(bot_api, config_repo);
    bot_svc
        .set_webhook(&base_url)
        .await
        .expect("failed to initialize bot service");

    start(AppState {
        download_svc,
        bot_svc,
    })
    .await;
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

async fn start(state: AppState) {
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
