use std::path;

use axum::{
    Json, Router,
    extract::{self, Request, State},
    response::{Html, IntoResponse, Response},
    routing,
};
use http::{HeaderMap, StatusCode, header::CONTENT_DISPOSITION};
use percent_encoding::NON_ALPHANUMERIC;
use tower::ServiceExt as _;
use tower_http::services::ServeDir;
use tracing::error;

use crate::{
    api::Update,
    repo::TaskStatus,
    service::{BotSvc, BotSvcError, DownloadSvc, TaskCreationParams},
};

#[derive(Clone)]
pub struct AppState {
    pub download_svc: DownloadSvc,
    pub bot_svc: BotSvc,
}

pub fn register(app: Router<AppState>) -> Router<AppState> {
    app.route("/", routing::get(INDEX))
        .route("/api/download", routing::post(create_task))
        .route("/api/download/{id}", routing::get(query_task))
        .route("/api/download/file/{id}", routing::get(download_file))
}

pub fn register_bot_webhook(app: Router<AppState>) -> Router<AppState> {
    app.route("/api/bot-updates", routing::post(listen_bot_updates))
}

const INDEX: Html<&[u8]> = Html(include_bytes!("../web/index.html"));

async fn create_task(
    State(AppState { download_svc, .. }): State<AppState>,
    Json(params): Json<TaskCreationParams>,
) -> impl IntoResponse {
    download_svc
        .create_task(&params)
        .await
        .map(Json)
        .map_err(AppError)
}

async fn query_task(
    State(AppState { download_svc, .. }): State<AppState>,
    extract::Path(id): extract::Path<String>,
) -> impl IntoResponse {
    download_svc
        .query_task(&id)
        .await
        .map(|result| NotFound(result.map(Json)))
        .map_err(AppError)
}

async fn download_file(
    State(AppState { download_svc, .. }): State<AppState>,
    extract::Path(id): extract::Path<String>,
    mut req: Request,
) -> impl IntoResponse {
    let task = match download_svc.query_task(&id).await {
        Ok(result) => match result {
            Some(task) => match task.status {
                TaskStatus::Pending => {
                    return (StatusCode::ACCEPTED, Json(task)).into_response();
                }
                TaskStatus::Done => task,
                TaskStatus::Error => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(task)).into_response();
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
                    percent_encoding::utf8_percent_encode(&task.message, NON_ALPHANUMERIC)
                ),
            )],
            res,
        )
            .into_response()
    }
}

async fn listen_bot_updates(
    State(AppState { bot_svc, .. }): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<Update>,
) -> Response {
    if let Err(e) = bot_svc
        .reply(
            &update,
            headers
                .get("X-Telegram-Bot-Api-Secret-Token")
                .map(|x| x.to_str().ok().unwrap_or_default())
                .unwrap_or_default(),
        )
        .await
    {
        return match e.downcast_ref::<BotSvcError>() {
            Some(e) => match e {
                BotSvcError::Unauthenticated => {
                    (StatusCode::UNAUTHORIZED, e.to_string()).into_response()
                }
            },
            None => AppError(e).into_response(),
        };
    }
    StatusCode::NO_CONTENT.into_response()
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
