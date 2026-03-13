use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite};

use super::db::get_db;
use crate::shared::get_millis;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegVersionItem {
    pub row_key: String,
    pub source: String,
    pub os: String,
    pub version: String,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
    pub arch: Option<String>,
    pub local_path: Option<String>,
    pub updated_at: i64,
    pub download_state: String,
    pub installed: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegVersionListResult {
    pub list: Vec<FfmpegVersionItem>,
    pub total: u64,
    pub has_more: bool,
    pub next_offset: u64,
}

fn build_row_key(
    source: &str,
    os: &str,
    version: &str,
    arch: Option<&str>,
    download_url: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        source,
        os,
        version,
        arch.unwrap_or(""),
        download_url.unwrap_or("")
    )
}

fn row_to_item(row: &sqlx::sqlite::SqliteRow) -> FfmpegVersionItem {
    FfmpegVersionItem {
        row_key: row.try_get::<String, _>("row_key").unwrap_or_default(),
        source: row.try_get::<String, _>("source").unwrap_or_default(),
        os: row.try_get::<String, _>("os").unwrap_or_default(),
        version: row.try_get::<String, _>("version").unwrap_or_default(),
        published_at: row.try_get::<Option<String>, _>("published_at").ok().flatten(),
        download_url: row.try_get::<Option<String>, _>("download_url").ok().flatten(),
        arch: row.try_get::<Option<String>, _>("arch").ok().flatten(),
        local_path: row.try_get::<Option<String>, _>("local_path").ok().flatten(),
        updated_at: row.try_get::<i64, _>("updated_at").unwrap_or_default(),
        download_state: row
            .try_get::<String, _>("download_state")
            .unwrap_or_else(|_| "not_downloaded".to_string()),
        installed: row.try_get::<i64, _>("installed").unwrap_or_default() != 0,
        is_active: row.try_get::<i64, _>("is_active").unwrap_or_default() != 0,
    }
}

pub async fn init() -> Result<()> {
    let pool = get_db().await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ffmpeg_versions (
            row_key TEXT PRIMARY KEY NOT NULL,
            source TEXT NOT NULL,
            os TEXT NOT NULL,
            version TEXT NOT NULL,
            published_at TEXT,
            download_url TEXT,
            arch TEXT,
            local_path TEXT,
            updated_at INTEGER NOT NULL,
            download_state TEXT NOT NULL DEFAULT 'not_downloaded',
            installed INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
    .execute(&pool)
    .await?;

    ensure_latest_schema().await?;
    seed_static_data().await?;
    Ok(())
}

async fn ensure_latest_schema() -> Result<()> {
    let pool = get_db().await?;
    let rows = sqlx::query("PRAGMA table_info('ffmpeg_versions');")
        .fetch_all(&pool)
        .await?;

    let columns = rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect::<Vec<_>>();

    let required = [
        "row_key",
        "source",
        "os",
        "version",
        "published_at",
        "download_url",
        "arch",
        "updated_at",
        "local_path",
        "download_state",
        "installed",
        "is_active",
    ];

    let up_to_date = required
        .iter()
        .all(|name| columns.iter().any(|col| col == name));

    if up_to_date {
        return Ok(());
    }

    let backup_table = format!("ffmpeg_versions_legacy_{}", get_millis());
    let rename_sql = format!("ALTER TABLE ffmpeg_versions RENAME TO {}", backup_table);
    sqlx::query(rename_sql.as_str()).execute(&pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE ffmpeg_versions (
            row_key TEXT PRIMARY KEY NOT NULL,
            source TEXT NOT NULL,
            os TEXT NOT NULL,
            version TEXT NOT NULL,
            published_at TEXT,
            download_url TEXT,
            arch TEXT,
            local_path TEXT,
            updated_at INTEGER NOT NULL,
            download_state TEXT NOT NULL DEFAULT 'not_downloaded',
            installed INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn seed_static_data() -> Result<usize> {
    let pool = get_db().await?;
    let static_rows = crate::services::ffmpeg_sources::list_versions(None, None, None, 5000, 0).list;
    let now = get_millis();
    let mut affected = 0usize;

    for row in static_rows {
        let row_key = build_row_key(
            row.source.as_str(),
            row.os.as_str(),
            row.version.as_str(),
            row.arch.as_deref(),
            row.download_url.as_deref(),
        );
        let result = sqlx::query(
            r#"
            INSERT INTO ffmpeg_versions
                (row_key, source, os, version, published_at, download_url, arch, local_path, updated_at, download_state, installed, is_active)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, 'not_downloaded', 0, 0)
            ON CONFLICT(row_key) DO UPDATE SET
                source = excluded.source,
                os = excluded.os,
                version = excluded.version,
                published_at = excluded.published_at,
                download_url = excluded.download_url,
                arch = excluded.arch,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(row_key)
        .bind(row.source)
        .bind(row.os)
        .bind(row.version)
        .bind(row.published_at)
        .bind(row.download_url)
        .bind(row.arch)
        .bind(now)
        .execute(&pool)
        .await?;

        affected += result.rows_affected() as usize;
    }

    Ok(affected)
}

pub async fn list(
    source: Option<String>,
    os: Option<String>,
    arch: Option<String>,
    keyword: Option<String>,
    limit: usize,
    offset: usize,
) -> Result<FfmpegVersionListResult> {
    let pool = get_db().await?;
    let limit = limit.clamp(1, 200);

    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT row_key, source, os, version, published_at, download_url, arch, local_path, updated_at, download_state, installed, is_active FROM ffmpeg_versions WHERE 1=1",
    );
    let mut count_query = QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(*) AS total FROM ffmpeg_versions WHERE 1=1",
    );

    if let Some(v) = source.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        query.push(" AND source = ").push_bind(v.clone());
        count_query.push(" AND source = ").push_bind(v);
    }
    if let Some(v) = os.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        query.push(" AND os = ").push_bind(v.clone());
        count_query.push(" AND os = ").push_bind(v);
    }
    if let Some(v) = arch
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
    {
        let normalized = match v.as_str() {
            "amd64" => "x86_64".to_string(),
            "aarch64" => "arm64".to_string(),
            other => other.to_string(),
        };
        query.push(" AND arch = ").push_bind(normalized.clone());
        count_query.push(" AND arch = ").push_bind(normalized);
    }
    if let Some(v) = keyword.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        let pattern = format!("%{}%", v);
        query
            .push(" AND (version LIKE ")
            .push_bind(pattern.clone())
            .push(" OR source LIKE ")
            .push_bind(pattern.clone())
            .push(" OR IFNULL(download_url, '') LIKE ")
            .push_bind(pattern.clone())
            .push(")");
        count_query
            .push(" AND (version LIKE ")
            .push_bind(pattern.clone())
            .push(" OR source LIKE ")
            .push_bind(pattern.clone())
            .push(" OR IFNULL(download_url, '') LIKE ")
            .push_bind(pattern.clone())
            .push(")");
    }

    query
        .push(" ORDER BY updated_at DESC, version DESC")
        .push(" LIMIT ")
        .push_bind(limit as i64)
        .push(" OFFSET ")
        .push_bind(offset as i64);

    let rows = query.build().fetch_all(&pool).await?;
    let total_row = count_query.build().fetch_one(&pool).await?;
    let total = total_row.try_get::<i64, _>("total").unwrap_or_default().max(0) as u64;

    let list = rows.iter().map(row_to_item).collect::<Vec<_>>();
    let next_offset = (offset + list.len()) as u64;
    Ok(FfmpegVersionListResult {
        list,
        total,
        has_more: next_offset < total,
        next_offset,
    })
}

pub async fn list_installed() -> Result<Vec<FfmpegVersionItem>> {
    let pool = get_db().await?;
    let rows = sqlx::query(
        r#"
        SELECT row_key, source, os, version, published_at, download_url, arch, updated_at, download_state, installed, is_active
             ,local_path
        FROM ffmpeg_versions
        WHERE installed = 1
        ORDER BY is_active DESC, updated_at DESC, version DESC
        "#,
    )
    .fetch_all(&pool)
    .await?;
    Ok(rows.iter().map(row_to_item).collect())
}

pub async fn set_download_state(row_key: &str, state: &str) -> Result<()> {
    let normalized = state.trim().to_ascii_lowercase();
    let (installed, is_active) = match normalized.as_str() {
        "downloading" => (0_i64, 0_i64),
        "downloaded" => (1_i64, 0_i64),
        "failed" | "not_downloaded" => (0_i64, 0_i64),
        _ => return Err(anyhow!("invalid download state: {}", state)),
    };

    let pool = get_db().await?;
    let affected = sqlx::query(
        r#"
        UPDATE ffmpeg_versions
        SET download_state = ?1, installed = ?2, is_active = ?3, updated_at = ?4
        WHERE row_key = ?5
        "#,
    )
    .bind(normalized)
    .bind(installed)
    .bind(is_active)
    .bind(get_millis())
    .bind(row_key)
    .execute(&pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(anyhow!("ffmpeg version not found"));
    }
    Ok(())
}

pub async fn activate(row_key: &str) -> Result<()> {
    let pool = get_db().await?;
    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE ffmpeg_versions SET is_active = 0 WHERE installed = 1")
        .execute(&mut *tx)
        .await?;

    let affected = sqlx::query(
        r#"
        UPDATE ffmpeg_versions
        SET installed = 1, is_active = 1, download_state = 'downloaded', updated_at = ?1
        WHERE row_key = ?2
        "#,
    )
    .bind(get_millis())
    .bind(row_key)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(anyhow!("ffmpeg version not found"));
    }

    tx.commit().await?;
    Ok(())
}

pub async fn remove_installation(row_key: &str) -> Result<()> {
    let pool = get_db().await?;
    let affected = sqlx::query(
        r#"
        UPDATE ffmpeg_versions
        SET installed = 0, is_active = 0, download_state = 'not_downloaded', updated_at = ?1
        WHERE row_key = ?2
        "#,
    )
    .bind(get_millis())
    .bind(row_key)
    .execute(&pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(anyhow!("ffmpeg version not found"));
    }
    Ok(())
}

pub async fn get_by_row_key(row_key: &str) -> Result<Option<FfmpegVersionItem>> {
    let pool = get_db().await?;
    let row = sqlx::query(
        r#"
        SELECT row_key, source, os, version, published_at, download_url, arch, local_path, updated_at, download_state, installed, is_active
        FROM ffmpeg_versions
        WHERE row_key = ?1
        LIMIT 1
        "#,
    )
    .bind(row_key)
    .fetch_optional(&pool)
    .await?;
    Ok(row.as_ref().map(row_to_item))
}

pub async fn mark_downloaded(row_key: &str, local_path: &str) -> Result<()> {
    let pool = get_db().await?;
    let affected = sqlx::query(
        r#"
        UPDATE ffmpeg_versions
        SET download_state = 'downloaded',
            installed = 1,
            is_active = 0,
            local_path = ?1,
            updated_at = ?2
        WHERE row_key = ?3
        "#,
    )
    .bind(local_path)
    .bind(get_millis())
    .bind(row_key)
    .execute(&pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(anyhow!("ffmpeg version not found"));
    }
    Ok(())
}
