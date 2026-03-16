use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Error, anyhow};
use reqwest::{Url, header};
use serde::{Deserialize, Serialize};
use tokio::{fs, task::JoinSet};
use tracing::instrument;
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    parser::{ParseResult, Registry},
    utils,
};

pub struct DownloadService {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    parser_registry: Arc<Registry>,
}

struct Task {
    id: String,
    result: Option<anyhow::Result<String>>,
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

#[derive(Debug, Serialize)]
pub struct TaskQueryResult {
    pub id: String,
    pub status: &'static str,
    pub message: String,
}

impl DownloadService {
    pub fn new(parser_registry: Arc<Registry>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            parser_registry,
        }
    }

    pub fn create_task(&self, params: &TaskCreationParams) -> TaskCreationResult {
        let id = Uuid::new_v4().to_string();
        self.tasks.write().unwrap().insert(
            id.clone(),
            Task {
                id: id.clone(),
                result: None,
            },
        );

        {
            let id = id.clone();
            let params = params.clone();
            let parser_registry = Arc::clone(&self.parser_registry);
            let tasks = Arc::clone(&self.tasks);

            tokio::spawn(async move {
                let result = Self::run_task(parser_registry, params).await;
                if let Err(e) = &result {
                    tracing::error!(task_id = %id, error = ?e, "task failed");
                }
                tasks
                    .write()
                    .unwrap()
                    .get_mut(&id)
                    .expect("unexpected task miss")
                    .result = Some(result);
            });
        }

        TaskCreationResult { id }
    }

    pub fn query_task(&self, id: &str) -> Option<TaskQueryResult> {
        self.tasks.read().unwrap().get(id).map(|task| {
            let (status, message) = match task.result.as_ref() {
                Some(result) => match result {
                    Ok(path) => ("done", path.clone()),
                    Err(e) => ("error", format!("{e:#}")),
                },
                None => ("pending", "".to_owned()),
            };
            TaskQueryResult {
                id: task.id.clone(),
                status,
                message,
            }
        })
    }

    pub fn download_file(&self) {}

    async fn run_task(
        parser_registry: Arc<Registry>,
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
                let path = format!("{title}.md");
                fs::write(&path, body).await?;
                Ok(path)
            }
            result @ ParseResult::Images { .. } => Self::save_images(result).await,
        }
    }

    #[instrument(level = "trace")]
    async fn save_images(result: ParseResult) -> anyhow::Result<String> {
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

        let path = format!("{title}.zip");
        let mut writer = ZipWriter::new(File::create(&path)?);
        let options = SimpleFileOptions::default();
        for (name, res) in results {
            writer.start_file(format!("{title}/{name}"), options)?;
            writer.write_all(&res)?;
        }
        writer.finish()?;

        Ok(path)
    }
}
