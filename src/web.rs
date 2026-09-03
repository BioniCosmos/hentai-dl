use std::sync::Arc;

use axum::Router;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::{parser, repo::TaskRepo, route, service::DownloadService};

pub async fn start() {
    tracing_subscriber::registry()
        .with(fmt::layer().pretty())
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("hentai_dl=trace,tower_http=trace,axum=trace"))
                .expect("failed to initialize the filter layer"),
        )
        .init();

    let conn =
        SqlitePool::connect(&dotenvy::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_owned()))
            .await
            .expect("failed to connect to database");
    let task_repo = TaskRepo::new(conn);

    let parser_registry = Arc::new(parser::init_registry());
    let download_svc = Arc::new(DownloadService::new(parser_registry, task_repo));

    let app = route::register(Router::new())
        .layer(TraceLayer::new_for_http())
        .with_state(download_svc);

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
