use super::db::{get_db, TableSpec};
use anyhow::Result;
use sea_query::{
    ColumnDef, Expr, Iden, OnConflict, Order, Query, SqliteQueryBuilder, Table,
    TableCreateStatement,
};
use sea_query_binder::SqlxBinder;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::shared::get_millis;

#[derive(Iden)]
pub enum TaskHistory {
    Table,
    Id,
    TaskType,
    Status,
    InputPath,
    OutputPath,
    CreatedAt,
    FinishedAt,
    ErrorMessage,
    TaskData,
    CommandLine,
}

pub struct TaskHistoryTable;

impl TableSpec for TaskHistoryTable {
    const NAME: &'static str = "task_history";
    const LATEST: i32 = 5;

    fn create_stmt() -> TableCreateStatement {
        Table::create()
            .table(TaskHistory::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(TaskHistory::Id)
                    .string()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(TaskHistory::TaskType).string().not_null())
            .col(ColumnDef::new(TaskHistory::Status).string().not_null())
            .col(ColumnDef::new(TaskHistory::InputPath).string().not_null())
            .col(ColumnDef::new(TaskHistory::OutputPath).string())
            .col(ColumnDef::new(TaskHistory::CreatedAt).integer().not_null())
            .col(ColumnDef::new(TaskHistory::FinishedAt).integer().not_null())
            .col(ColumnDef::new(TaskHistory::ErrorMessage).string())
            .col(ColumnDef::new(TaskHistory::TaskData).text().not_null())
            .col(ColumnDef::new(TaskHistory::CommandLine).text())
            .to_owned()
    }

    fn check_latest() -> impl std::future::Future<Output = Result<()>> {
        async {
            super::db::init_meta().await?;
            let pool = get_db().await?;
            let cur = super::db::get_version(Self::NAME).await?;

            if cur < Self::LATEST {
                sqlx::query("DROP TABLE IF EXISTS task_history")
                    .execute(&pool)
                    .await?;
            }

            let create_sql = Self::create_stmt().to_string(SqliteQueryBuilder);
            sqlx::query(&create_sql).execute(&pool).await?;
            super::db::set_version(Self::NAME, Self::LATEST).await?;
            Ok(())
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskHistoryItem {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub input_path: String,
    pub output_path: Option<String>,
    pub created_at: i64,
    pub finished_at: i64,
    pub error_message: Option<String>,
    #[serde(skip_serializing)]
    pub task_data: String,
    #[serde(skip_serializing)]
    pub command_line: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MyFileItem {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub input_path: String,
    pub output_path: Option<String>,
    pub created_at: i64,
    pub finished_at: i64,
    pub error_message: Option<String>,
    pub is_favorite: bool,
}

pub async fn add_history(item: &TaskHistoryItem) -> Result<()> {
    let pool = get_db().await?;

    let (sql, values) = Query::insert()
        .into_table(TaskHistory::Table)
        .columns([
            TaskHistory::Id,
            TaskHistory::TaskType,
            TaskHistory::Status,
            TaskHistory::InputPath,
            TaskHistory::OutputPath,
            TaskHistory::CreatedAt,
            TaskHistory::FinishedAt,
            TaskHistory::ErrorMessage,
            TaskHistory::TaskData,
            TaskHistory::CommandLine,
        ])
        .values([
            item.id.clone().into(),
            item.task_type.clone().into(),
            item.status.clone().into(),
            item.input_path.clone().into(),
            item.output_path.clone().into(),
            item.created_at.into(),
            item.finished_at.into(),
            item.error_message.clone().into(),
            item.task_data.clone().into(),
            item.command_line.clone().into(),
        ])?
        .on_conflict(
            OnConflict::column(TaskHistory::Id)
                .update_columns([
                    TaskHistory::TaskType,
                    TaskHistory::Status,
                    TaskHistory::InputPath,
                    TaskHistory::OutputPath,
                    TaskHistory::CreatedAt,
                    TaskHistory::FinishedAt,
                    TaskHistory::ErrorMessage,
                    TaskHistory::TaskData,
                    TaskHistory::CommandLine,
                ])
                .to_owned(),
        )
        .build_sqlx(SqliteQueryBuilder);

    sqlx::query_with(&sql, values).execute(&pool).await?;
    sqlx::query(
        r#"
        DELETE FROM task_history
        WHERE id NOT IN (
            SELECT id
            FROM task_history
            ORDER BY finished_at DESC
            LIMIT 2000
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn get_history(
    limit: usize,
    offset: usize,
    task_type: Option<String>,
    keyword: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<Vec<TaskHistoryItem>> {
    let pool = get_db().await?;

    let mut query = Query::select();
    let sort_order = match sort_order
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("asc") => Order::Asc,
        _ => Order::Desc,
    };

    query
        .columns([
            TaskHistory::Id,
            TaskHistory::TaskType,
            TaskHistory::Status,
            TaskHistory::InputPath,
            TaskHistory::OutputPath,
            TaskHistory::CreatedAt,
            TaskHistory::FinishedAt,
            TaskHistory::ErrorMessage,
        ])
        .from(TaskHistory::Table)
        .limit(limit as u64)
        .offset(offset as u64);

    match sort_by
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("output_name") | Some("name") => {
            query
                .order_by(TaskHistory::OutputPath, sort_order.clone())
                .order_by(TaskHistory::Id, sort_order.clone());
        }
        _ => {
            query
                .order_by(TaskHistory::CreatedAt, sort_order.clone())
                .order_by(TaskHistory::Id, sort_order.clone());
        }
    }

    if let Some(t_type) = task_type {
        if t_type == "convert" {
            query.and_where(Expr::col(TaskHistory::TaskType).like("convert-%"));
        } else if t_type == "compress" {
            query.and_where(Expr::col(TaskHistory::TaskType).like("compress-%"));
        } else {
            query.and_where(Expr::col(TaskHistory::TaskType).eq(t_type));
        }
    }

    if let Some(raw) = keyword {
        let keyword = raw.trim().to_string();
        if !keyword.is_empty() {
            let pattern = format!("%{}%", keyword);
            query.and_where(
                Expr::col(TaskHistory::InputPath)
                    .like(&pattern)
                    .or(Expr::col(TaskHistory::OutputPath).like(&pattern)),
            );
        }
    }

    let (sql, values) = query.build_sqlx(SqliteQueryBuilder);
    let rows = sqlx::query_with(&sql, values).fetch_all(&pool).await?;

    let mut items = Vec::new();
    for row in rows {
        items.push(TaskHistoryItem {
            id: row.try_get("id")?,
            task_type: row.try_get("task_type")?,
            status: row.try_get("status")?,
            input_path: row.try_get("input_path")?,
            output_path: row.try_get("output_path")?,
            created_at: row.try_get("created_at")?,
            finished_at: row.try_get("finished_at")?,
            error_message: row.try_get("error_message")?,
            task_data: String::new(),
            command_line: None,
        });
    }

    Ok(items)
}

pub async fn get_my_files(
    limit: usize,
    offset: usize,
    keyword: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    _media_type: Option<String>,
) -> Result<Vec<MyFileItem>> {
    let pool = get_db().await?;
    let mut query = Query::select();

    query
        .columns([
            TaskHistory::Id,
            TaskHistory::TaskType,
            TaskHistory::Status,
            TaskHistory::InputPath,
            TaskHistory::OutputPath,
            TaskHistory::CreatedAt,
            TaskHistory::FinishedAt,
            TaskHistory::ErrorMessage,
        ])
        .from(TaskHistory::Table);

    query.and_where(Expr::col(TaskHistory::Status).eq("finished"));
    query.and_where(Expr::col(TaskHistory::OutputPath).is_not_null());

    let sort_order = match sort_order
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("asc") => Order::Asc,
        _ => Order::Desc,
    };

    match sort_by
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("name") => {
            query
                .order_by(TaskHistory::OutputPath, sort_order.clone())
                .order_by(TaskHistory::InputPath, sort_order.clone())
                .order_by(TaskHistory::Id, sort_order);
        }
        _ => {
            query
                .order_by(TaskHistory::CreatedAt, sort_order.clone())
                .order_by(TaskHistory::Id, sort_order);
        }
    }

    query.limit(limit as u64).offset(offset as u64);

    if let Some(raw) = keyword {
        let keyword = raw.trim().to_string();
        if !keyword.is_empty() {
            let pattern = format!("%{}%", keyword);
            query.and_where(
                Expr::col(TaskHistory::InputPath)
                    .like(&pattern)
                    .or(Expr::col(TaskHistory::OutputPath).like(&pattern)),
            );
        }
    }

    let (sql, values) = query.build_sqlx(SqliteQueryBuilder);
    let rows = sqlx::query_with(&sql, values).fetch_all(&pool).await?;

    let mut items = Vec::new();
    for row in rows {
        items.push(MyFileItem {
            id: row.try_get("id")?,
            task_type: row.try_get("task_type")?,
            status: row.try_get("status")?,
            input_path: row.try_get("input_path")?,
            output_path: row.try_get("output_path")?,
            created_at: row.try_get("created_at")?,
            finished_at: row.try_get("finished_at")?,
            error_message: row.try_get("error_message")?,
            is_favorite: false,
        });
    }

    Ok(items)
}

pub async fn delete_history(id: &str) -> Result<()> {
    let pool = get_db().await?;
    let (sql, values) = Query::delete()
        .from_table(TaskHistory::Table)
        .and_where(Expr::col(TaskHistory::Id).eq(id))
        .build_sqlx(SqliteQueryBuilder);

    sqlx::query_with(&sql, values).execute(&pool).await?;
    Ok(())
}

pub async fn clear_history(task_type: Option<String>) -> Result<()> {
    let pool = get_db().await?;
    let mut query = Query::delete();
    query.from_table(TaskHistory::Table);

    if let Some(t_type) = task_type {
        query.and_where(Expr::col(TaskHistory::TaskType).eq(t_type));
    }

    let (sql, values) = query.build_sqlx(SqliteQueryBuilder);
    sqlx::query_with(&sql, values).execute(&pool).await?;
    Ok(())
}

pub async fn cleanup_stale_processing(max_age_ms: i64) -> Result<u64> {
    let pool = get_db().await?;
    let now = get_millis();
    let cutoff = now - max_age_ms;
    let result = sqlx::query(
        r#"
        UPDATE task_history
        SET status = 'error',
            error_message = 'Task interrupted',
            finished_at = ?
        WHERE status IN ('processing', 'idle')
          AND created_at < ?
        "#,
    )
    .bind(now)
    .bind(cutoff)
    .execute(&pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn init() -> Result<()> {
    TaskHistoryTable::check_latest().await
}
