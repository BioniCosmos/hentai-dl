use serde::Serialize;
use sqlx::prelude::Type;

use crate::Context;

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

mod task {
    use super::*;

    pub async fn create(ctx: &Context, task: &Task) -> anyhow::Result<()> {
        sqlx::query!(
            "INSERT INTO tasks VALUES (?, ?, ?)",
            task.id,
            task.status,
            task.message,
        )
        .execute(&ctx.db)
        .await
        .and(Ok(()))
        .map_err(anyhow::Error::new)
    }

    pub async fn query(ctx: &Context, id: &str) -> anyhow::Result<Option<Task>> {
        sqlx::query_as!(
            Task,
            r#"SELECT id, status as "status: _", message FROM tasks WHERE id = ?"#,
            id,
        )
        .fetch_optional(&ctx.db)
        .await
        .map_err(anyhow::Error::new)
    }

    pub async fn update(ctx: &Context, id: &str, task: &TaskUpdate) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE tasks SET status = COALESCE(?, status), message = COALESCE(?, message) WHERE id = ?",
            task.status,
            task.message,
            id,
        )
        .execute(&ctx.db)
        .await
        .and(Ok(()))
        .map_err(anyhow::Error::new)
    }
}

mod config {
    use super::*;

    pub async fn query(ctx: &Context, key: &str) -> anyhow::Result<String> {
        sqlx::query_scalar!("SELECT value FROM configs WHERE key = ?", key)
            .fetch_optional(&ctx.db)
            .await
            .map(|x| x.unwrap_or_default())
            .map_err(anyhow::Error::new)
    }

    pub async fn set(ctx: &Context, key: &str, value: &str) -> anyhow::Result<()> {
        sqlx::query!(
            "INSERT INTO configs VALUES (?, ?) ON CONFLICT DO UPDATE SET value = ?",
            key,
            value,
            value
        )
        .execute(&ctx.db)
        .await
        .and(Ok(()))
        .map_err(anyhow::Error::new)
    }
}
