use std::{
    error,
    fmt::{self, Display, Formatter},
    io::{self, ErrorKind, Read, Write},
    sync::Arc,
};

use anyhow::{Context, anyhow};
use http::header;
use opendal::Operator;
use rand::distr::{Alphanumeric, SampleString as _};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinSet, try_join};
use tracing::error;
use url::Url;
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    api::{BotAPI, Update},
    parser::{ParseResult, Registry},
    repo::{ConfigRepo, Task, TaskRepo, TaskStatus, TaskUpdate},
    utils,
};

#[derive(Clone)]
pub struct DownloadSvc {
    parser_registry: Arc<Registry>,
    task_repo: TaskRepo,
    op: Operator,
}

#[derive(Clone, Deserialize)]
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

impl DownloadSvc {
    pub fn new(parser_registry: Arc<Registry>, task_repo: TaskRepo, op: Operator) -> Self {
        Self {
            parser_registry,
            task_repo,
            op,
        }
    }

    pub async fn create_task(
        &self,
        params: &TaskCreationParams,
    ) -> anyhow::Result<TaskCreationResult> {
        let id = Uuid::now_v7().to_string();
        self.task_repo
            .create(&Task {
                id: id.clone(),
                status: TaskStatus::Pending,
                message: String::new(),
            })
            .await?;

        {
            let id = id.clone();
            let params = params.clone();
            let svc = self.clone();

            tokio::spawn(async move {
                let result = svc.run_task(&id, params).await;
                let (status, message) = match result {
                    Ok(file_name) => ("done", file_name),
                    Err(error) => {
                        error!(task_id = id, ?error, "task failed");
                        ("error", format!("{error:#}"))
                    }
                };

                svc.task_repo
                    .update(
                        &id,
                        &TaskUpdate {
                            status: Some(status),
                            message: Some(message),
                        },
                    )
                    .await
            });
        }

        Ok(TaskCreationResult { id })
    }

    pub async fn query_task(&self, id: &str) -> anyhow::Result<Option<TaskQueryResult>> {
        self.task_repo.query(id).await
    }

    async fn run_task(&self, id: &str, params: TaskCreationParams) -> anyhow::Result<String> {
        let (url, raw) = match params {
            TaskCreationParams::Url { ref url } => (url, reqwest::get(url).await?.text().await?),
            TaskCreationParams::Raw { ref url, raw } => (url, raw),
        };
        match self
            .parser_registry
            .get(Url::parse(url)?.host_str().context("invalid url")?)
            .context("unsupported origin")?
            .parse(&raw)?
        {
            ParseResult::Markdown { title, body } => {
                self.op.write(&format!("{id}.md"), body).await?;
                Ok(format!("{title}.md"))
            }
            result @ ParseResult::Images { .. } => self.save_images(id, result).await,
        }
    }

    async fn save_images(&self, id: &str, result: ParseResult) -> anyhow::Result<String> {
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
            .collect::<anyhow::Result<Vec<_>>>()?;
        results.sort();

        let mut writer = self.op.writer(&format!("{id}.zip")).await?;
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

        Ok(title + ".zip")
    }
}

macro_rules! get {
    ($r:expr) => {{
        use crate::api::Response::*;

        match $r {
            Success(result) => result,
            Failure {
                error_code,
                description,
            } => anyhow::bail!("bot API error {error_code}: {description}"),
        }
    }};
}

#[derive(Clone)]
pub struct BotSvc {
    api: BotAPI,
    repo: ConfigRepo,
}

#[derive(Debug)]
pub enum BotSvcError {
    Unauthenticated,
}

impl error::Error for BotSvcError {}

impl Display for BotSvcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use BotSvcError::*;

        match self {
            Unauthenticated => write!(f, "Unauthorized request. The secret token is invalid."),
        }
    }
}

impl BotSvc {
    pub fn new(api: BotAPI, repo: ConfigRepo) -> Self {
        Self { api, repo }
    }

    pub async fn set_webhook(&self, base_url: &str) -> anyhow::Result<()> {
        let webhook_url = Url::parse(base_url)?.join("/api/bot-updates")?;

        let info = get!(self.api.get_webhook_info().await?);
        if info.url == webhook_url.as_str() {
            return Ok(());
        }

        get!(self.api.delete_webhook().await?);

        let secret = Alphanumeric.sample_string(&mut rand::rng(), 256);
        get!(self.api.set_webhook(webhook_url.as_str(), &secret).await?);
        self.repo.set("secret", &secret).await
    }

    pub async fn reply(&self, update: &Update, secret: &str) -> anyhow::Result<()> {
        if secret != self.repo.query("secret").await? {
            return Err(anyhow::Error::new(BotSvcError::Unauthenticated));
        }

        get!(
            self.api
                .send_message(
                    update.message.as_ref().unwrap().chat.id,
                    update.message.as_ref().unwrap().text.as_ref().unwrap(),
                )
                .await?
        );
        Ok(())
    }
}
