use crate::common::errors::DomainError;
use crate::domain::entities::task::{ScheduledTask, TaskExecution, TaskStatus};
use chrono::{DateTime, NaiveTime, Utc};
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub trait TaskRepository: Send + Sync + 'static {
    async fn list_tasks(&self) -> Result<Vec<ScheduledTask>, DomainError>;

    async fn get_task_by_type(&self, task_type: &str)
    -> Result<Option<ScheduledTask>, DomainError>;

    async fn get_task(&self, id: Uuid) -> Result<Option<ScheduledTask>, DomainError>;

    async fn update_task(
        &self,
        id: Uuid,
        enabled: Option<bool>,
        trigger_type: Option<&str>,
        schedule_interval_seconds: Option<i32>,
        schedule_time: Option<NaiveTime>,
        schedule_day_of_week: Option<i16>,
        config: Option<&serde_json::Value>,
    ) -> Result<(), DomainError>;

    async fn set_task_status(&self, id: Uuid, status: TaskStatus) -> Result<(), DomainError>;

    async fn update_last_run(
        &self,
        id: Uuid,
        duration_secs: i32,
        status: TaskStatus,
        message: Option<&str>,
        next_run_at: Option<DateTime<Utc>>,
    ) -> Result<(), DomainError>;

    async fn increment_run_counters(&self, id: Uuid, success: bool) -> Result<(), DomainError>;

    async fn get_due_tasks(&self) -> Result<Vec<ScheduledTask>, DomainError>;

    async fn create_execution(
        &self,
        task_id: Uuid,
        triggered_by: &str,
    ) -> Result<Uuid, DomainError>;

    async fn complete_execution(
        &self,
        execution_id: Uuid,
        status: TaskStatus,
        message: Option<&str>,
        result: Option<&serde_json::Value>,
        error_details: Option<&str>,
    ) -> Result<(), DomainError>;

    async fn get_task_executions(
        &self,
        task_id: Uuid,
        limit: i32,
    ) -> Result<Vec<TaskExecution>, DomainError>;

    async fn get_audio_files_count(&self) -> Result<i64, DomainError>;

    async fn get_audio_files_without_metadata(&self) -> Result<i64, DomainError>;
}
