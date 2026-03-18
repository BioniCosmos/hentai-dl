use std::{path, sync::Arc};

use axum::{
    Json, Router,
    extract::{self, Request, State},
    response::{Html, IntoResponse},
    routing,
};
use http::{StatusCode, header::CONTENT_DISPOSITION};
use percent_encoding::NON_ALPHANUMERIC;
use tokio::net::TcpListener;
use tower::ServiceExt;
use tower_http::{services::ServeDir, trace::TraceLayer};
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
        .route("/", routing::get(Html(include_bytes!("../web/index.html"))))
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
                async |State(download_svc): State<Arc<DownloadService>>,
                       extract::Path(id): extract::Path<String>| {
                    download_svc
                        .query_task(&id)
                        .map(Json)
                        .ok_or(StatusCode::NOT_FOUND)
                },
            ),
        )
        .route(
            "/api/download/file/{id}",
            routing::get(
                async |State(download_svc): State<Arc<DownloadService>>,
                       extract::Path(id): extract::Path<String>,
                       mut req: Request| {
                    // TODO: check task status
                    let Some(task) = download_svc.query_task(&id) else {
                        return StatusCode::NOT_FOUND.into_response();
                    };

                    *req.uri_mut() = format!(
                        "{}.{}",
                        &req.uri().path()["/api/download/file".len()..],
                        path::Path::new(&task.message)
                            .extension()
                            .expect("unexpected invalid file name")
                            .display()
                    )
                    .parse()
                    .expect("unexpected invalid path");

                    let srv = ServeDir::new(".");
                    let res = srv.oneshot(req).await.unwrap();
                    if res.status() != StatusCode::OK {
                        res.into_response()
                    } else {
                        (
                            [(
                                CONTENT_DISPOSITION,
                                format!(
                                    "attachment; filename*=UTF-8''{}",
                                    percent_encoding::utf8_percent_encode(
                                        &task.message,
                                        NON_ALPHANUMERIC
                                    )
                                ),
                            )],
                            res,
                        )
                            .into_response()
                    }
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
