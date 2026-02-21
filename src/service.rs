use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Error, anyhow, bail};
use reqwest::Url;
use tokio::{fs, task::JoinSet};
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::parser::{ParseResult, Registry};

pub struct Download {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    parser_registry: Arc<Registry>,
}

struct Task {
    id: String,
    result: Option<anyhow::Result<String>>,
}

#[derive(Clone)]
pub struct TaskCreationParams {
    pub param_type: String,
    pub url: String,
    pub raw: String,
}

pub struct TaskCreationResult {
    pub id: String,
}

#[derive(Debug)]
pub struct TaskQueryResult {
    pub id: String,
    pub status: &'static str,
    pub message: String,
}

impl Download {
    pub fn new(parser_registry: Arc<Registry>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            parser_registry,
        }
    }

    pub fn create_task(&mut self, params: &TaskCreationParams) -> TaskCreationResult {
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
                tasks
                    .write()
                    .unwrap()
                    .get_mut(&id)
                    .expect("unexpected task miss")
                    .result = Some(Self::run_task(parser_registry, params).await);
            });
        }

        TaskCreationResult { id }
    }

    pub fn query_task(&self, id: &str) -> Option<TaskQueryResult> {
        self.tasks.read().unwrap().get(id).map(|task| {
            let (status, message) = match task.result.as_ref() {
                Some(result) => match result {
                    Ok(path) => ("done", path.clone()),
                    Err(e) => ("error", e.to_string()),
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
        match parser_registry
            .get(Url::parse(&params.url)?.host_str().context("invalid url")?)
            .context("unsupported origin")?
            .parse(&match params.param_type.as_str() {
                "url" => reqwest::get(&params.url).await?.text().await?,
                "raw" => params.raw.clone(),
                _ => bail!("invalid param type"),
            })? {
            ParseResult::Markdown { title, body } => {
                let path = format!("{title}.md");
                fs::write(format!("{title}.md"), body).await?;
                Ok(path)
            }
            result @ ParseResult::Images { .. } => Self::save_images(result).await,
        }
    }

    // TODO: get the extension by `Content-Type`
    async fn save_images(result: ParseResult) -> anyhow::Result<String> {
        let ParseResult::Images { title, urls } = result else {
            return Err(anyhow!("wrong param: expecting `ParseResult::Images`"));
        };

        let width = {
            let mut n = urls.len();
            if n == 0 {
                1
            } else {
                let mut count = 0;
                while n > 0 {
                    count += 1;
                    n /= 10;
                }
                count
            }
        };
        let mut tasks = JoinSet::new();
        for (i, url) in urls.into_iter().enumerate() {
            tasks.spawn(async move {
                Ok((
                    format!(
                        "{i:0>width$}{}",
                        Path::new(&url)
                            .extension()
                            .map(|ext| format!(".{}", ext.display()))
                            .unwrap_or_default()
                    ),
                    reqwest::get(&url).await?.bytes().await?,
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
