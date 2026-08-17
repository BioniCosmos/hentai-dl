use std::{fs::File, io::Write, sync::Arc};

use anyhow::{Context, Error, anyhow};
use http::header;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::{fs, task::JoinSet};
use tracing::instrument;
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    parser::{ParseResult, Registry},
    repo::{Task, TaskRepo, TaskStatus, TaskUpdate},
    utils,
};

pub struct DownloadService {
    parser_registry: Arc<Registry>,
    task_repo: TaskRepo,
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

impl DownloadService {
    pub fn new(parser_registry: Arc<Registry>, task_repo: TaskRepo) -> Self {
        Self {
            parser_registry,
            task_repo,
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
            let parser_registry = Arc::clone(&self.parser_registry);
            let task_repo = self.task_repo.clone();

            tokio::spawn(async move {
                let result = Self::run_task(parser_registry, &id, params).await;
                let (status, message) = match result {
                    Ok(file_name) => ("done", file_name),
                    Err(error) => {
                        tracing::error!(task_id = %id, ?error, "task failed");
                        ("error", format!("{error:#}"))
                    }
                };

                task_repo
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

    async fn run_task(
        parser_registry: Arc<Registry>,
        id: &str,
        params: TaskCreationParams,
    ) -> anyhow::Result<String> {
        let (url, raw) = match params {
            TaskCreationParams::Url { ref url } => (url, reqwest::get(url).await?.text().await?),
            TaskCreationParams::Raw { ref url, raw } => (url, raw),
        };
        match parser_registry
            .get(Url::parse(url)?.host_str().context("invalid url")?)
            .context("unsupported origin")?
            .parse(&raw)?
        {
            ParseResult::Markdown { title, body } => {
                fs::write(format!("{id}.md"), body).await?;
                Ok(format!("{title}.md"))
            }
            result @ ParseResult::Images { .. } => Self::save_images(id, result).await,
        }
    }

    #[instrument(level = "trace")]
    async fn save_images(id: &str, result: ParseResult) -> anyhow::Result<String> {
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
                    .and_then(|media_type| media_type.to_str().map_err(Error::new))?;
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
            .collect::<Result<Vec<_>, Error>>()?;
        results.sort();

        let mut writer = ZipWriter::new(File::create(format!("{id}.zip"))?);
        let options = SimpleFileOptions::default();
        for (name, res) in results {
            writer.start_file(format!("{title}/{name}"), options)?;
            writer.write_all(&res)?;
        }
        writer.finish()?;

        Ok(title + ".zip")
    }
}
