use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::atomic::{AtomicU64, Ordering};

use super::db::get_db;
use crate::shared::get_millis;

static FAVORITE_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteCommandItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub command: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row_to_item(row: &sqlx::sqlite::SqliteRow) -> FavoriteCommandItem {
    FavoriteCommandItem {
        id: row.try_get::<String, _>("id").unwrap_or_default(),
        title: row.try_get::<String, _>("title").unwrap_or_default(),
        description: row.try_get::<Option<String>, _>("description").ok().flatten(),
        command: row.try_get::<String, _>("command").unwrap_or_default(),
        created_at: row.try_get::<i64, _>("created_at").unwrap_or_default(),
        updated_at: row.try_get::<i64, _>("updated_at").unwrap_or_default(),
    }
}

fn new_id() -> String {
    let seq = FAVORITE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("fav-{}-{}", get_millis(), seq)
}

pub async fn init() -> Result<()> {
    let pool = get_db().await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS favorite_commands (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            command TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    let columns = sqlx::query("PRAGMA table_info('favorite_commands');")
        .fetch_all(&pool)
        .await?
        .iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect::<Vec<_>>();

    if !columns.iter().any(|c| c == "description") {
        sqlx::query("ALTER TABLE favorite_commands ADD COLUMN description TEXT")
            .execute(&pool)
            .await
            .ok();
    }

    Ok(())
}

pub async fn list(limit: usize, offset: usize) -> Result<Vec<FavoriteCommandItem>> {
    let pool = get_db().await?;
    let rows = sqlx::query(
        r#"
        SELECT id, title, description, command, created_at, updated_at
        FROM favorite_commands
        ORDER BY updated_at DESC, created_at DESC, id DESC
        LIMIT ?1 OFFSET ?2
        "#,
    )
    .bind(limit.clamp(1, 200) as i64)
    .bind(offset as i64)
    .fetch_all(&pool)
    .await?;

    Ok(rows.iter().map(row_to_item).collect())
}

pub async fn create(
    title: String,
    description: Option<String>,
    command: String,
) -> Result<FavoriteCommandItem> {
    let pool = get_db().await?;
    let now = get_millis();
    let item = FavoriteCommandItem {
        id: new_id(),
        title,
        description,
        command,
        created_at: now,
        updated_at: now,
    };

    sqlx::query(
        r#"
        INSERT INTO favorite_commands (id, title, description, command, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(item.id.as_str())
    .bind(item.title.as_str())
    .bind(item.description.as_deref())
    .bind(item.command.as_str())
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(&pool)
    .await?;

    Ok(item)
}

pub async fn delete(id: &str) -> Result<()> {
    let pool = get_db().await?;
    sqlx::query("DELETE FROM favorite_commands WHERE id = ?1")
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(())
}
