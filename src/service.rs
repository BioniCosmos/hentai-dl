use std::io::{self, ErrorKind, Read as _, Write as _};

use anyhow::{Context as _, Result, anyhow};
use http::header;
use rand::distr::{Alphanumeric, SampleString as _};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::oneshot::{self, Sender},
    task::JoinSet,
    try_join,
};
use tracing::{Instrument, Span, error, field::Empty, info_span, instrument, warn};
use url::Url;
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    Context,
    api::{self, Update},
    parser::ParseResult,
    repo::{Task, TaskStatus, TaskUpdate, config as config_repo, task as task_repo},
    utils,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "paramType", rename_all = "camelCase")]
pub enum TaskCreationParams {
    Url { url: String },
    Raw { url: String, raw: String },
}

#[derive(Serialize)]
pub struct TaskCreationResult {
    pub id: String,
}

type TaskQueryResult = Task;

pub mod download {
    use super::*;

    #[instrument(err(Debug), skip_all, fields(params = ?params, task_id))]
    pub async fn create_task(
        ctx: &Context,
        params: &TaskCreationParams,
        tx: Option<Sender<Task>>,
    ) -> Result<TaskCreationResult> {
        let id = Uuid::now_v7().to_string();
        Span::current().record("task_id", &id);
        task_repo::create(
            ctx,
            &Task {
                id: id.clone(),
                file: String::new(),
                status: TaskStatus::Pending,
                message: String::new(),
            },
        )
        .await?;

        {
            let id = id.clone();
            let params = params.clone();
            let ctx = ctx.clone();

            tokio::spawn(
                async move {
                    let task = run_task(&ctx, &id, params).await.unwrap_or_else(|error| {
                        error!(?error, "task failed");
                        Task {
                            id,
                            file: String::new(),
                            status: TaskStatus::Error,
                            message: format!("{error:#}"),
                        }
                    });
                    if let Err(error) = task_repo::update(
                        &ctx,
                        &task.id,
                        &TaskUpdate {
                            file: Some(&task.file),
                            status: Some(task.status),
                            message: Some(&task.message),
                        },
                    )
                    .await
                    {
                        error!(?error, "failed to update task status in database",);
                    }
                    if let Some(tx) = tx
                        && let Err(_) = tx.send(task)
                    {
                        error!("failed to send task status signal");
                    }
                }
                .in_current_span(),
            );
        }

        Ok(TaskCreationResult { id })
    }

    pub async fn query_task(ctx: &Context, id: &str) -> Result<Option<TaskQueryResult>> {
        task_repo::query(ctx, id).await
    }

    async fn run_task(ctx: &Context, id: &str, params: TaskCreationParams) -> Result<Task> {
        let (url, raw) = match params {
            TaskCreationParams::Url { ref url } => (url, reqwest::get(url).await?.text().await?),
            TaskCreationParams::Raw { ref url, raw } => (url, raw),
        };
        let (file, title) = match ctx
            .parsers
            .get(Url::parse(url)?.host_str().context("invalid url")?)
            .context("unsupported origin")?
            .parse(&raw)?
        {
            ParseResult::Markdown { title, body } => {
                let file = format!("{id}.md");
                ctx.store.write(&file, body).await?;
                (file, title + ".md")
            }
            result @ ParseResult::Images { .. } => save_images(ctx, id, result).await?,
        };
        Ok(Task {
            id: id.to_owned(),
            file,
            status: TaskStatus::Done,
            message: title,
        })
    }

    async fn save_images(ctx: &Context, id: &str, result: ParseResult) -> Result<(String, String)> {
        let ParseResult::Images { title, urls } = result else {
            return Err(anyhow!("wrong param: expecting `ParseResult::Images`"));
        };

        let width = (urls.len().checked_ilog10().unwrap_or_default() + 1) as usize;
        let mut tasks = JoinSet::new();
        for (i, url) in urls.into_iter().enumerate() {
            tasks.spawn(async move {
                let response = reqwest::get(&url).await?;
                let content_type = response
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .context("`Content-Type` header not found")
                    .and_then(|media_type| media_type.to_str().map_err(anyhow::Error::new))?;
                Ok((
                    format!(
                        "{i:0>width$}.{}",
                        utils::media_type_to_ext(content_type)
                            .context(format!("unsupported `Content-Type`: {content_type}"))?
                    ),
                    response.bytes().await?,
                ))
            });
        }

        let mut results = tasks
            .join_all()
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        results.sort();

        let file = format!("{id}.zip");
        let mut writer = ctx.store.writer(&file).await?;
        let (mut rx, tx) = io::pipe()?;

        let compress_task = async {
            let title_clone = title.clone();
            tokio::task::spawn_blocking(move || {
                let mut writer = ZipWriter::new_stream(tx);
                let options = SimpleFileOptions::default();
                for (name, res) in results {
                    writer.start_file(format!("{title_clone}/{name}"), options)?;
                    writer.write_all(&res)?;
                }
                writer.finish()
            })
            .await??;
            anyhow::Ok(())
        };

        let save_task = async {
            loop {
                let mut buf = vec![0u8; 1024 * 64];
                match rx.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.truncate(n);
                        writer.write(buf).await?
                    }
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(anyhow::Error::new(e)),
                }
            }
            Ok(())
        };

        if let Err(e) = try_join!(compress_task, save_task) {
            writer.abort().await?;
            return Err(e);
        }
        writer.close().await?;

        Ok((file, title + ".zip"))
    }
}

pub mod bot {
    use super::{download as download_svc, *};

    macro_rules! map {
        ($r:expr) => {{
            use crate::api::Response::*;

            match $r {
                Success(result) => Ok(result),
                Failure {
                    error_code,
                    description,
                } => Err(anyhow!("bot API error {error_code}: {description}")),
            }
        }};
    }

    #[derive(Debug)]
    pub enum Error {
        Unauthenticated,
        InvalidPayload,
        NoSignal,
        Internal,
    }

    impl std::error::Error for Error {}

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            use Error::*;

            match self {
                Unauthenticated => write!(f, "Unauthorized request. The secret token is invalid."),
                InvalidPayload => write!(f, "invalid payload"),
                NoSignal => write!(f, "The corresponding task signal sender has been dropped."),
                Internal => write!(f, "internal server error"),
            }
        }
    }

    pub async fn set_webhook(ctx: &Context, base_url: &str) -> Result<()> {
        let webhook_url = Url::parse(base_url)?.join("/api/bot-updates")?;

        let info = map!(api::get_webhook_info(ctx).await?)?;
        if info.url == webhook_url.as_str() {
            return Ok(());
        }

        map!(api::delete_webhook(ctx).await?)?;

        let secret = Alphanumeric.sample_string(&mut rand::rng(), 256);
        map!(api::set_webhook(ctx, webhook_url.as_str(), &secret).await?)?;
        config_repo::set(ctx, "secret", &secret).await
    }

    #[instrument(err(Debug), skip(ctx))]
    pub async fn reply(ctx: Context, update: Update, secret: &str) -> Result<()> {
        if secret != config_repo::query(&ctx, "secret").await? {
            return Err(anyhow::Error::new(Error::Unauthenticated));
        }
        let Some(message) = update.message else {
            return Err(anyhow::Error::new(Error::InvalidPayload));
        };

        let span = info_span!("bot-message-handler", task_id = Empty);
        let s = span.clone();
        tokio::spawn(
            async move {
                let chat_id = message.chat.id;

                let handle = async || {
                    let message = message
                        .text
                        .context("empty message (or unsupported message type)")?;
                    let (tx, rx) = oneshot::channel();

                    let TaskCreationResult { id } = download_svc::create_task(
                        &ctx,
                        &TaskCreationParams::Url { url: message },
                        Some(tx),
                    )
                    .await
                    .context(Error::Internal)?;
                    s.record("task_id", &id);

                    let task = rx.await.context(Error::NoSignal)?;
                    match task.status {
                        TaskStatus::Done => {
                            let meta = ctx
                                .store
                                .stat(&task.file)
                                .await
                                .context("failed to read file metadata")
                                .context(Error::Internal)?;
                            let stream = ctx
                                .store
                                .reader(&task.file)
                                .await
                                .context("failed to get file reader")
                                .context(Error::Internal)?
                                .into_bytes_stream(..)
                                .await
                                .context("failed to get file stream")
                                .context(Error::Internal)?;
                            map!(
                                api::send_document(
                                    &ctx,
                                    chat_id,
                                    &task.message,
                                    meta.content_length(),
                                    stream
                                )
                                .await
                                .context(Error::Internal)?
                            )
                            .context(Error::Internal)?;
                        }
                        TaskStatus::Error => return Err(anyhow!(task.message)),
                        _ => unreachable!(),
                    }
                    Ok(())
                };

                if let Err(error) = handle().await {
                    error!(?error, "failed to handle bot message");
                    if let Err(error) = api::send_message(
                        &ctx,
                        chat_id,
                        &if error.is::<Error>() {
                            "internal server error".to_owned()
                        } else {
                            error.to_string()
                        },
                    )
                    .await
                    {
                        warn!(?error, "failed to send back error message to user");
                    }
                }
            }
            .instrument(span),
        );

        Ok(())
    }
}
