use serde::Serialize;
use sqlx::{SqlitePool, prelude::Type};

#[derive(Clone)]
pub struct TaskRepo(SqlitePool);

#[derive(Serialize)]
pub struct Task {
    pub id: String,
    pub status: TaskStatus,
    pub message: String,
}

#[derive(Serialize, Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Done,
    Error,
}

pub struct TaskUpdate {
    pub status: Option<&'static str>,
    pub message: Option<String>,
}

impl TaskRepo {
    pub fn new(conn: SqlitePool) -> Self {
        TaskRepo(conn)
    }

    pub async fn create(&self, task: &Task) -> anyhow::Result<()> {
        sqlx::query!(
            "INSERT INTO tasks VALUES (?, ?, ?)",
            task.id,
            task.status,
            task.message,
        )
        .execute(&self.0)
        .await
        .and(Ok(()))
        .map_err(anyhow::Error::new)
    }

    pub async fn query(&self, id: &str) -> anyhow::Result<Option<Task>> {
        sqlx::query_as!(
            Task,
            r#"SELECT id, status as "status: _", message FROM tasks WHERE id = ?"#,
            id,
        )
        .fetch_optional(&self.0)
        .await
        .map_err(anyhow::Error::new)
    }

    pub async fn update(&self, id: &str, task: &TaskUpdate) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE tasks SET status = COALESCE(?, status), message = COALESCE(?, message) WHERE id = ?",
            task.status,
            task.message,
            id,
        )
        .execute(&self.0)
        .await
        .and(Ok(()))
        .map_err(anyhow::Error::new)
    }
}
