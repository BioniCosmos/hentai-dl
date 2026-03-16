use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    response::Html,
    routing,
};
use reqwest::StatusCode;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::{
    parser,
    service::{DownloadService, TaskCreationParams},
};

pub async fn start() {
    tracing_subscriber::registry()
        .with(fmt::layer().pretty())
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("hentai_dl=trace,tower_http=trace,axum=trace"))
                .expect("failed to initialize the filter layer"),
        )
        .init();

    let parser_registry = Arc::new(parser::init_registry());
    let download_svc = Arc::new(DownloadService::new(parser_registry));

    let app = Router::new()
        .route("/", routing::get(Html(include_bytes!("../index.html"))))
        .route(
            "/api/download",
            routing::post(
                async |State(download_svc): State<Arc<DownloadService>>,
                       Json(params): Json<TaskCreationParams>| {
                    Json(download_svc.create_task(&params))
                },
            ),
        )
        .route(
            "/api/download/{id}",
            routing::get(
                async |State(download_svc): State<Arc<DownloadService>>, Path(id): Path<String>| {
                    download_svc
                        .query_task(&id)
                        .map(Json)
                        .ok_or(StatusCode::NOT_FOUND)
                },
            ),
        )
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
