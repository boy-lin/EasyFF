use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::atomic::{AtomicU64, Ordering};

use super::db::get_db;
use crate::shared::get_millis;

static FAVORITE_SEQ: AtomicU64 = AtomicU64::new(0);

pub const SYNC_STATE_SYNCED: &str = "synced";
pub const SYNC_STATE_PENDING_UPSERT: &str = "pending_upsert";
pub const SYNC_STATE_PENDING_DELETE: &str = "pending_delete";
pub const SYNC_STATE_CONFLICT: &str = "conflict";

const DEFAULT_FAVORITE_COMMANDS: [(&str, &str, &str); 10] = [
    (
        "压缩视频(H.264)",
        "常规压缩，兼顾体积与兼容性",
        r#"ffmpeg -i input.mp4 -c:v libx264 -preset medium -crf 23 -c:a aac -b:a 128k output.mp4"#,
    ),
    (
        "高压缩视频(H.265)",
        "更高压缩率，适合存档",
        r#"ffmpeg -i input.mp4 -c:v libx265 -preset medium -crf 28 -c:a aac -b:a 128k output.mp4"#,
    ),
    (
        "转码为 MP4",
        "将常见格式统一转码为 MP4",
        r#"ffmpeg -i input.mkv -c:v libx264 -preset fast -crf 22 -c:a aac -movflags +faststart output.mp4"#,
    ),
    (
        "提取视频音频(MP3)",
        "从视频中提取 MP3 音频",
        r#"ffmpeg -i input.mp4 -vn -c:a libmp3lame -q:a 2 output.mp3"#,
    ),
    (
        "提取无损音频(WAV)",
        "从视频中提取无损 WAV 音频",
        r#"ffmpeg -i input.mp4 -vn -acodec pcm_s16le -ar 44100 -ac 2 output.wav"#,
    ),
    (
        "视频转 GIF",
        "常用调色板流程，画质更好",
        r#"ffmpeg -i input.mp4 -vf "fps=12,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse" output.gif"#,
    ),
    (
        "裁剪视频时长",
        "从指定时间截取固定时长",
        r#"ffmpeg -ss 00:00:10 -i input.mp4 -t 00:00:15 -c copy output.mp4"#,
    ),
    (
        "视频缩放到 720p",
        "保持比例缩放，高度对齐",
        r#"ffmpeg -i input.mp4 -vf scale=-2:720 -c:v libx264 -crf 22 -preset fast -c:a copy output_720p.mp4"#,
    ),
    (
        "合并视频与字幕",
        "把 SRT 字幕硬编码进视频",
        r#"ffmpeg -i input.mp4 -vf subtitles=subtitle.srt -c:v libx264 -crf 22 -preset medium -c:a copy output_sub.mp4"#,
    ),
    (
        "视频静音导出",
        "去除音频仅保留画面",
        r#"ffmpeg -i input.mp4 -c:v libx264 -crf 23 -preset fast -an output_silent.mp4"#,
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteCommandItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub command: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub sync_state: String,
    pub server_version: Option<i64>,
    pub updated_by_device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteCommandSyncAck {
    pub id: String,
    pub server_version: Option<i64>,
    pub updated_at: Option<i64>,
    pub deleted_at: Option<i64>,
    pub updated_by_device_id: Option<String>,
}

fn row_to_item(row: &sqlx::sqlite::SqliteRow) -> FavoriteCommandItem {
    FavoriteCommandItem {
        id: row.try_get::<String, _>("id").unwrap_or_default(),
        title: row.try_get::<String, _>("title").unwrap_or_default(),
        description: row.try_get::<Option<String>, _>("description").ok().flatten(),
        command: row.try_get::<String, _>("command").unwrap_or_default(),
        created_at: row.try_get::<i64, _>("created_at").unwrap_or_default(),
        updated_at: row.try_get::<i64, _>("updated_at").unwrap_or_default(),
        deleted_at: row.try_get::<Option<i64>, _>("deleted_at").ok().flatten(),
        sync_state: row
            .try_get::<Option<String>, _>("sync_state")
            .ok()
            .flatten()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| SYNC_STATE_SYNCED.to_string()),
        server_version: row.try_get::<Option<i64>, _>("server_version").ok().flatten(),
        updated_by_device_id: row
            .try_get::<Option<String>, _>("updated_by_device_id")
            .ok()
            .flatten(),
    }
}

fn new_id() -> String {
    let seq = FAVORITE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("fav-{}-{}", get_millis(), seq)
}

async fn ensure_column(pool: &sqlx::SqlitePool, columns: &[String], name: &str, sql: &str) {
    if !columns.iter().any(|c| c == name) {
        let _ = sqlx::query(sql).execute(pool).await;
    }
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
            updated_at INTEGER NOT NULL,
            deleted_at INTEGER,
            sync_state TEXT NOT NULL DEFAULT 'synced',
            server_version INTEGER,
            updated_by_device_id TEXT
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS favorite_sync_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
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

    ensure_column(
        &pool,
        &columns,
        "description",
        "ALTER TABLE favorite_commands ADD COLUMN description TEXT",
    )
    .await;
    ensure_column(
        &pool,
        &columns,
        "deleted_at",
        "ALTER TABLE favorite_commands ADD COLUMN deleted_at INTEGER",
    )
    .await;
    ensure_column(
        &pool,
        &columns,
        "sync_state",
        "ALTER TABLE favorite_commands ADD COLUMN sync_state TEXT NOT NULL DEFAULT 'synced'",
    )
    .await;
    ensure_column(
        &pool,
        &columns,
        "server_version",
        "ALTER TABLE favorite_commands ADD COLUMN server_version INTEGER",
    )
    .await;
    ensure_column(
        &pool,
        &columns,
        "updated_by_device_id",
        "ALTER TABLE favorite_commands ADD COLUMN updated_by_device_id TEXT",
    )
    .await;

    sqlx::query(
        "UPDATE favorite_commands SET sync_state = 'synced' WHERE sync_state IS NULL OR TRIM(sync_state) = ''",
    )
    .execute(&pool)
    .await
    .ok();

    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM favorite_commands")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    if count == 0 {
        let now = get_millis();
        for (idx, (title, description, command)) in DEFAULT_FAVORITE_COMMANDS.iter().enumerate() {
            let ts = now + (DEFAULT_FAVORITE_COMMANDS.len() - idx) as i64;
            sqlx::query(
                r#"
                INSERT INTO favorite_commands (
                    id, title, description, command, created_at, updated_at, deleted_at, sync_state, server_version, updated_by_device_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 'synced', NULL, NULL)
                "#,
            )
            .bind(new_id())
            .bind(*title)
            .bind(*description)
            .bind(*command)
            .bind(ts)
            .bind(ts)
            .execute(&pool)
            .await?;
        }
    }

    Ok(())
}

pub async fn list(limit: usize, offset: usize) -> Result<Vec<FavoriteCommandItem>> {
    list_for_sync(limit, offset, false).await
}

pub async fn list_for_sync(
    limit: usize,
    offset: usize,
    include_deleted: bool,
) -> Result<Vec<FavoriteCommandItem>> {
    let pool = get_db().await?;
    let sql = if include_deleted {
        r#"
        SELECT id, title, description, command, created_at, updated_at, deleted_at, sync_state, server_version, updated_by_device_id
        FROM favorite_commands
        ORDER BY updated_at DESC, created_at DESC, id DESC
        LIMIT ?1 OFFSET ?2
        "#
    } else {
        r#"
        SELECT id, title, description, command, created_at, updated_at, deleted_at, sync_state, server_version, updated_by_device_id
        FROM favorite_commands
        WHERE deleted_at IS NULL
        ORDER BY updated_at DESC, created_at DESC, id DESC
        LIMIT ?1 OFFSET ?2
        "#
    };

    let rows = sqlx::query(sql)
        .bind(limit.clamp(1, 500) as i64)
        .bind(offset as i64)
        .fetch_all(&pool)
        .await?;

    Ok(rows.iter().map(row_to_item).collect())
}

pub async fn list_pending_sync(limit: usize) -> Result<Vec<FavoriteCommandItem>> {
    let pool = get_db().await?;
    let rows = sqlx::query(
        r#"
        SELECT id, title, description, command, created_at, updated_at, deleted_at, sync_state, server_version, updated_by_device_id
        FROM favorite_commands
        WHERE sync_state IN ('pending_upsert', 'pending_delete')
        ORDER BY updated_at ASC, id ASC
        LIMIT ?1
        "#,
    )
    .bind(limit.clamp(1, 1000) as i64)
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
        deleted_at: None,
        sync_state: SYNC_STATE_PENDING_UPSERT.to_string(),
        server_version: None,
        updated_by_device_id: None,
    };

    sqlx::query(
        r#"
        INSERT INTO favorite_commands (
            id, title, description, command, created_at, updated_at, deleted_at, sync_state, server_version, updated_by_device_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(item.id.as_str())
    .bind(item.title.as_str())
    .bind(item.description.as_deref())
    .bind(item.command.as_str())
    .bind(item.created_at)
    .bind(item.updated_at)
    .bind(item.deleted_at)
    .bind(item.sync_state.as_str())
    .bind(item.server_version)
    .bind(item.updated_by_device_id.as_deref())
    .execute(&pool)
    .await?;

    Ok(item)
}

pub async fn delete(id: &str) -> Result<()> {
    let pool = get_db().await?;
    let now = get_millis();
    sqlx::query(
        r#"
        UPDATE favorite_commands
        SET deleted_at = ?2,
            updated_at = ?2,
            sync_state = 'pending_delete'
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .bind(now)
    .execute(&pool)
    .await?;
    Ok(())
}

pub async fn upsert_from_remote_batch(items: Vec<FavoriteCommandItem>) -> Result<usize> {
    if items.is_empty() {
        return Ok(0);
    }

    let pool = get_db().await?;
    let mut tx = pool.begin().await?;
    let mut applied = 0_usize;

    for item in items {
        let row = sqlx::query(
            r#"
            SELECT updated_at, server_version
            FROM favorite_commands
            WHERE id = ?1
            "#,
        )
        .bind(item.id.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        let should_apply = if let Some(r) = row {
            let local_updated = r.try_get::<i64, _>("updated_at").unwrap_or_default();
            let local_version = r.try_get::<Option<i64>, _>("server_version").ok().flatten().unwrap_or(0);
            let remote_version = item.server_version.unwrap_or(0);
            item.updated_at > local_updated
                || (item.updated_at == local_updated && remote_version >= local_version)
        } else {
            true
        };

        if !should_apply {
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO favorite_commands (
                id, title, description, command, created_at, updated_at, deleted_at, sync_state, server_version, updated_by_device_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'synced', ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                command = excluded.command,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at,
                sync_state = 'synced',
                server_version = excluded.server_version,
                updated_by_device_id = excluded.updated_by_device_id
            "#,
        )
        .bind(item.id)
        .bind(item.title)
        .bind(item.description)
        .bind(item.command)
        .bind(item.created_at)
        .bind(item.updated_at)
        .bind(item.deleted_at)
        .bind(item.server_version)
        .bind(item.updated_by_device_id)
        .execute(&mut *tx)
        .await?;

        applied += 1;
    }

    tx.commit().await?;
    Ok(applied)
}

pub async fn mark_synced(acks: Vec<FavoriteCommandSyncAck>) -> Result<usize> {
    if acks.is_empty() {
        return Ok(0);
    }

    let pool = get_db().await?;
    let mut tx = pool.begin().await?;
    let mut updated = 0_usize;

    for ack in acks {
        let result = sqlx::query(
            r#"
            UPDATE favorite_commands
            SET sync_state = 'synced',
                server_version = COALESCE(?2, server_version),
                updated_at = COALESCE(?3, updated_at),
                deleted_at = COALESCE(?4, deleted_at),
                updated_by_device_id = COALESCE(?5, updated_by_device_id)
            WHERE id = ?1
            "#,
        )
        .bind(ack.id)
        .bind(ack.server_version)
        .bind(ack.updated_at)
        .bind(ack.deleted_at)
        .bind(ack.updated_by_device_id)
        .execute(&mut *tx)
        .await?;

        updated += result.rows_affected() as usize;
    }

    tx.commit().await?;
    Ok(updated)
}

pub async fn get_sync_cursor() -> Result<i64> {
    let pool = get_db().await?;
    let value = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value FROM favorite_sync_meta WHERE key = 'favorite_commands_cursor'",
    )
    .fetch_optional(&pool)
    .await?
    .flatten()
    .unwrap_or_else(|| "0".to_string());

    Ok(value.parse::<i64>().unwrap_or(0))
}

pub async fn set_sync_cursor(cursor: i64) -> Result<()> {
    let pool = get_db().await?;
    sqlx::query(
        r#"
        INSERT INTO favorite_sync_meta(key, value)
        VALUES('favorite_commands_cursor', ?1)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        "#,
    )
    .bind(cursor.to_string())
    .execute(&pool)
    .await?;
    Ok(())
}
