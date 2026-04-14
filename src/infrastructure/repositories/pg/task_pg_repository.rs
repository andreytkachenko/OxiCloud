use chrono::{DateTime, NaiveTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::common::errors::{DomainError, ErrorKind};
use crate::domain::entities::task::{ScheduledTask, TaskExecution, TaskStatus, TaskType, TriggerType};
use crate::domain::repositories::task_repository::TaskRepository;

pub struct TaskPgRepository {
    pool: Arc<PgPool>,
}

impl TaskPgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn row_to_task(row: TaskRow) -> ScheduledTask {
        ScheduledTask {
            id: row.id,
            task_type: TaskType::try_from_str(&row.task_type)
                .unwrap_or(TaskType::AudioMetadataExtraction),
            name: row.name,
            description: row.description,
            enabled: row.enabled,
            status: TaskStatus::try_from_str(&row.status).unwrap_or(TaskStatus::Inactive),
            trigger_type: TriggerType::try_from_str(&row.trigger_type)
                .unwrap_or(TriggerType::Manual),
            schedule_interval_seconds: row.schedule_interval_seconds,
            schedule_time: row.schedule_time,
            schedule_day_of_week: row.schedule_day_of_week,
            last_run_at: row.last_run_at,
            last_run_duration_secs: row.last_run_duration_secs,
            last_run_status: row
                .last_run_status
                .map(|s| TaskStatus::try_from_str(&s).unwrap_or(TaskStatus::Inactive)),
            last_run_message: row.last_run_message,
            next_run_at: row.next_run_at,
            total_runs: row.total_runs,
            total_successes: row.total_successes,
            total_failures: row.total_failures,
            created_at: row.created_at,
            updated_at: row.updated_at,
            created_by: row.created_by,
            config: row.config,
        }
    }

    fn row_to_execution(row: TaskExecutionRow) -> TaskExecution {
        TaskExecution {
            id: row.id,
            task_id: row.task_id,
            started_at: row.started_at,
            completed_at: row.completed_at,
            duration_secs: row.duration_secs,
            status: TaskStatus::try_from_str(&row.status).unwrap_or(TaskStatus::Inactive),
            message: row.message,
            result: row.result,
            triggered_by: row.triggered_by,
            error_details: row.error_details,
        }
    }
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    task_type: String,
    name: String,
    description: Option<String>,
    enabled: bool,
    status: String,
    trigger_type: String,
    schedule_interval_seconds: Option<i32>,
    schedule_time: Option<NaiveTime>,
    schedule_day_of_week: Option<i16>,
    last_run_at: Option<DateTime<Utc>>,
    last_run_duration_secs: Option<i32>,
    last_run_status: Option<String>,
    last_run_message: Option<String>,
    next_run_at: Option<DateTime<Utc>>,
    total_runs: i32,
    total_successes: i32,
    total_failures: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    created_by: Option<Uuid>,
    config: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct TaskExecutionRow {
    id: Uuid,
    task_id: Uuid,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    duration_secs: Option<i32>,
    status: String,
    message: Option<String>,
    result: Option<serde_json::Value>,
    triggered_by: String,
    error_details: Option<String>,
}

impl TaskRepository for TaskPgRepository {
    async fn list_tasks(&self) -> Result<Vec<ScheduledTask>, DomainError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, task_type::text as task_type, name, description, enabled, 
                   status::text as status, trigger_type::text as trigger_type,
                   schedule_interval_seconds, schedule_time, schedule_day_of_week,
                   last_run_at, last_run_duration_secs, last_run_status::text as last_run_status, last_run_message,
                   next_run_at, total_runs, total_successes, total_failures,
                   created_at, updated_at, created_by, config
            FROM tasks.scheduled_tasks
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(rows.into_iter().map(Self::row_to_task).collect())
    }

    async fn get_task_by_type(
        &self,
        task_type: &str,
    ) -> Result<Option<ScheduledTask>, DomainError> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, task_type::text as task_type, name, description, enabled, 
                   status::text as status, trigger_type::text as trigger_type,
                   schedule_interval_seconds, schedule_time, schedule_day_of_week,
                   last_run_at, last_run_duration_secs, last_run_status::text as last_run_status, last_run_message,
                   next_run_at, total_runs, total_successes, total_failures,
                   created_at, updated_at, created_by, config
            FROM tasks.scheduled_tasks
            WHERE task_type = $1
            "#,
        )
        .bind(task_type)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(row.map(Self::row_to_task))
    }

    async fn get_task(&self, id: Uuid) -> Result<Option<ScheduledTask>, DomainError> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, task_type::text as task_type, name, description, enabled, 
                   status::text as status, trigger_type::text as trigger_type,
                   schedule_interval_seconds, schedule_time, schedule_day_of_week,
                   last_run_at, last_run_duration_secs, last_run_status::text as last_run_status, last_run_message,
                   next_run_at, total_runs, total_successes, total_failures,
                   created_at, updated_at, created_by, config
            FROM tasks.scheduled_tasks
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(row.map(Self::row_to_task))
    }

    async fn update_task(
        &self,
        id: Uuid,
        enabled: Option<bool>,
        trigger_type: Option<&str>,
        schedule_interval_seconds: Option<i32>,
        schedule_time: Option<NaiveTime>,
        schedule_day_of_week: Option<i16>,
        config: Option<&serde_json::Value>,
    ) -> Result<(), DomainError> {
        let mut query_parts = vec!["updated_at = NOW()".to_string()];
        let mut param_count = 1;

        if enabled.is_some() {
            query_parts.push(format!("enabled = ${}", param_count));
            param_count += 1;
        }
        if trigger_type.is_some() {
            query_parts.push(format!("trigger_type = ${}::tasks.trigger_type", param_count));
            param_count += 1;
        }
        if schedule_interval_seconds.is_some() {
            query_parts.push(format!("schedule_interval_seconds = ${}", param_count));
            param_count += 1;
        }
        if schedule_time.is_some() {
            query_parts.push(format!("schedule_time = ${}", param_count));
            param_count += 1;
        }
        if schedule_day_of_week.is_some() {
            query_parts.push(format!("schedule_day_of_week = ${}", param_count));
            param_count += 1;
        }
        if config.is_some() {
            query_parts.push(format!("config = ${}", param_count));
            param_count += 1;
        }

        let query = format!(
            "UPDATE tasks.scheduled_tasks SET {} WHERE id = ${}",
            query_parts.join(", "),
            param_count
        );

        let mut q = sqlx::query(&query);

        if let Some(e) = enabled {
            q = q.bind(e);
        }
        if let Some(st) = trigger_type {
            q = q.bind(st);
        }
        if let Some(interval) = schedule_interval_seconds {
            q = q.bind(interval);
        }
        if let Some(time) = schedule_time {
            q = q.bind(time);
        }
        if let Some(day) = schedule_day_of_week {
            q = q.bind(day);
        }
        if let Some(cfg) = config {
            q = q.bind(cfg);
        }
        q = q.bind(id);

        q.execute(self.pool.as_ref()).await.map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(())
    }

    async fn set_task_status(&self, id: Uuid, status: TaskStatus) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE tasks.scheduled_tasks SET status = $1::tasks.task_status, updated_at = NOW() WHERE id = $2",
        )
        .bind(status.as_str())
        .bind(id)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(())
    }

    async fn update_last_run(
        &self,
        id: Uuid,
        duration_secs: i32,
        status: TaskStatus,
        message: Option<&str>,
        next_run_at: Option<DateTime<Utc>>,
    ) -> Result<(), DomainError> {
        sqlx::query(
            r#"
            UPDATE tasks.scheduled_tasks 
            SET last_run_at = NOW(),
                last_run_duration_secs = $1,
                last_run_status = $2::tasks.task_status,
                last_run_message = $3,
                next_run_at = $4,
                updated_at = NOW()
            WHERE id = $5
            "#,
        )
        .bind(duration_secs)
        .bind(status.as_str())
        .bind(message)
        .bind(next_run_at)
        .bind(id)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(())
    }

    async fn increment_run_counters(&self, id: Uuid, success: bool) -> Result<(), DomainError> {
        let increment = if success {
            "total_successes = total_successes + 1"
        } else {
            "total_failures = total_failures + 1"
        };

        let query = format!(
            "UPDATE tasks.scheduled_tasks SET total_runs = total_runs + 1, {} WHERE id = $1",
            increment
        );

        sqlx::query(&query)
            .bind(id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| {
                DomainError::new(
                    ErrorKind::InternalError,
                    "TaskRepository",
                    format!("DB error: {}", e),
                )
            })?;

        Ok(())
    }

    async fn get_due_tasks(&self) -> Result<Vec<ScheduledTask>, DomainError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, task_type::text as task_type, name, description, enabled, 
                   status::text as status, trigger_type::text as trigger_type,
                   schedule_interval_seconds, schedule_time, schedule_day_of_week,
                   last_run_at, last_run_duration_secs, last_run_status::text as last_run_status, last_run_message,
                   next_run_at, total_runs, total_successes, total_failures,
                   created_at, updated_at, created_by, config
            FROM tasks.scheduled_tasks
            WHERE enabled = TRUE
              AND (trigger_type::text = 'periodic' AND (next_run_at IS NULL OR next_run_at <= NOW()))
            ORDER BY next_run_at ASC NULLS FIRST
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(rows.into_iter().map(Self::row_to_task).collect())
    }

    async fn create_execution(
        &self,
        task_id: Uuid,
        triggered_by: &str,
    ) -> Result<Uuid, DomainError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO tasks.task_executions (task_id, triggered_by, status, started_at)
            VALUES ($1, $2, 'running', NOW())
            RETURNING id
            "#,
        )
        .bind(task_id)
        .bind(triggered_by)
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(id)
    }

    async fn complete_execution(
        &self,
        execution_id: Uuid,
        status: TaskStatus,
        message: Option<&str>,
        result: Option<&serde_json::Value>,
        error_details: Option<&str>,
    ) -> Result<(), DomainError> {
        sqlx::query(
            r#"
            UPDATE tasks.task_executions
            SET completed_at = NOW(),
                duration_secs = EXTRACT(EPOCH FROM (NOW() - started_at))::INTEGER,
                status = $1::tasks.task_status,
                message = $2,
                result = $3,
                error_details = $4
            WHERE id = $5
            "#,
        )
        .bind(status.as_str())
        .bind(message)
        .bind(result)
        .bind(error_details)
        .bind(execution_id)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(())
    }

    async fn get_task_executions(
        &self,
        task_id: Uuid,
        limit: i32,
    ) -> Result<Vec<TaskExecution>, DomainError> {
        let rows = sqlx::query_as::<_, TaskExecutionRow>(
            r#"
            SELECT id, task_id, started_at, completed_at, duration_secs, status::text as status,
                   message, result, triggered_by, error_details
            FROM tasks.task_executions
            WHERE task_id = $1
            ORDER BY started_at DESC
            LIMIT $2
            "#,
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(rows.into_iter().map(Self::row_to_execution).collect())
    }

    async fn get_audio_files_count(&self) -> Result<i64, DomainError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM storage.files WHERE mime_type LIKE 'audio/%'",
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(count)
    }

    async fn get_audio_files_without_metadata(&self) -> Result<i64, DomainError> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint 
            FROM storage.files f
            WHERE f.mime_type LIKE 'audio/%'
              AND NOT EXISTS (
                  SELECT 1 FROM audio.file_metadata m WHERE m.file_id = f.id
              )
            "#,
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(|e| {
            DomainError::new(
                ErrorKind::InternalError,
                "TaskRepository",
                format!("DB error: {}", e),
            )
        })?;

        Ok(count)
    }
}
