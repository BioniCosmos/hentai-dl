use std::{path, sync::Arc};

use axum::{
    Json, Router,
    extract::{self, Request, State},
    response::{Html, IntoResponse, Response},
    routing,
};
use http::{StatusCode, header::CONTENT_DISPOSITION};
use percent_encoding::NON_ALPHANUMERIC;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower::ServiceExt;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::{
    parser,
    repo::{TaskRepo, TaskStatus},
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

    let conn =
        SqlitePool::connect(&dotenvy::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_owned()))
            .await
            .expect("failed to connect to database");
    let task_repo = TaskRepo::new(conn);

    let parser_registry = Arc::new(parser::init_registry());
    let download_svc = Arc::new(DownloadService::new(parser_registry, task_repo));

    let app = Router::new()
        .route("/", routing::get(Html(include_bytes!("../web/index.html"))))
        .route(
            "/api/download",
            routing::post(
                async |State(download_svc): State<Arc<DownloadService>>,
                       Json(params): Json<TaskCreationParams>| {
                    download_svc
                        .create_task(&params)
                        .await
                        .map(Json)
                        .map_err(AppError)
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
                        .await
                        .map(|result| NotFound(result.map(Json)))
                        .map_err(AppError)
                },
            ),
        )
        .route(
            "/api/download/file/{id}",
            routing::get(
                async |State(download_svc): State<Arc<DownloadService>>,
                       extract::Path(id): extract::Path<String>,
                       mut req: Request| {
                    let task = match download_svc.query_task(&id).await {
                        Ok(result) => match result {
                            Some(task) => match task.status {
                                TaskStatus::Pending => {
                                    return (StatusCode::ACCEPTED, Json(task)).into_response();
                                }
                                TaskStatus::Done => task,
                                TaskStatus::Error => {
                                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(task))
                                        .into_response();
                                }
                            },
                            None => return StatusCode::NOT_FOUND.into_response(),
                        },
                        Err(e) => return AppError(e).into_response(),
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

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!(error = ?self.0);
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

struct NotFound<T: IntoResponse>(Option<T>);

impl<T: IntoResponse> IntoResponse for NotFound<T> {
    fn into_response(self) -> Response {
        match self.0 {
            Some(result) => result.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}
