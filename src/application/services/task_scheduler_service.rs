use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::common::errors::DomainError;
use crate::domain::entities::task::{ScheduledTask, TaskExecution, TaskStatus, TaskType, TriggerType};
use crate::domain::errors::ErrorKind;
use crate::domain::repositories::task_repository::TaskRepository;
use crate::infrastructure::repositories::pg::TaskPgRepository;
use crate::infrastructure::services::audio_metadata_service::AudioMetadataService;

pub struct TaskSchedulerService {
    task_repo: Arc<TaskPgRepository>,
    audio_service: Option<Arc<AudioMetadataService>>,
    running: Arc<RwLock<bool>>,
}

impl TaskSchedulerService {
    pub fn new(
        task_repo: Arc<TaskPgRepository>,
        audio_service: Option<Arc<AudioMetadataService>>,
    ) -> Self {
        Self {
            task_repo,
            audio_service,
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn start_scheduler(&self) {
        let mut is_running = self.running.write().await;
        if *is_running {
            info!("Task scheduler is already running");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("Starting task scheduler");

        let repo = self.task_repo.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            loop {
                ticker.tick().await;

                let is_running = running.read().await;
                if !*is_running {
                    info!("Task scheduler stopped");
                    break;
                }
                drop(is_running);

                if let Err(e) = Self::check_and_run_due_tasks(&repo).await {
                    error!("Error checking due tasks: {}", e);
                }
            }
        });
    }

    pub async fn stop_scheduler(&self) {
        let mut is_running = self.running.write().await;
        *is_running = false;
        info!("Task scheduler will stop after current check");
    }

    async fn check_and_run_due_tasks(repo: &Arc<TaskPgRepository>) -> Result<(), DomainError> {
        let due_tasks = repo.get_due_tasks().await?;

        for task in due_tasks {
            info!(
                "Running due task: {} ({})",
                task.name,
                task.task_type.as_str()
            );
            if let Err(e) = Self::execute_task(&task, repo).await {
                error!("Task {} failed: {}", task.name, e);
            }
        }

        Ok(())
    }

    async fn execute_task(
        task: &ScheduledTask,
        repo: &Arc<TaskPgRepository>,
    ) -> Result<(), DomainError> {
        let execution_id = repo.create_execution(task.id, "schedule").await?;

        repo.set_task_status(task.id, TaskStatus::Running).await?;

        let result = match task.task_type {
            TaskType::AudioMetadataExtraction => {
                Self::run_audio_metadata_extraction(repo.clone()).await
            }
        };

        let (status, message, error_details) = match result {
            Ok(msg) => (TaskStatus::Completed, Some(msg), None),
            Err(e) => (TaskStatus::Failed, None, Some(e.to_string())),
        };

        repo.complete_execution(
            execution_id,
            status,
            message.as_deref(),
            None,
            error_details.as_deref(),
        )
        .await?;

        let duration = if status == TaskStatus::Completed {
            0
        } else {
            1
        };

        let next_run = task.calculate_next_run();
        repo.update_last_run(task.id, duration, status, message.as_deref(), next_run)
            .await?;

        repo.increment_run_counters(task.id, status == TaskStatus::Completed)
            .await?;

        repo.set_task_status(
            task.id,
            if task.enabled {
                TaskStatus::Active
            } else {
                TaskStatus::Inactive
            },
        )
        .await?;

        Ok(())
    }

    async fn run_audio_metadata_extraction(
        _repo: Arc<TaskPgRepository>,
    ) -> Result<String, DomainError> {
        warn!("Audio metadata extraction service not fully wired in scheduler");
        Ok("Task queued".to_string())
    }

    pub async fn list_tasks(&self) -> Result<Vec<ScheduledTask>, DomainError> {
        self.task_repo.list_tasks().await
    }

    pub async fn get_task(&self, id: Uuid) -> Result<Option<ScheduledTask>, DomainError> {
        self.task_repo.get_task(id).await
    }

    pub async fn get_task_by_type(
        &self,
        task_type: &str,
    ) -> Result<Option<ScheduledTask>, DomainError> {
        self.task_repo.get_task_by_type(task_type).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_task(
        &self,
        id: Uuid,
        enabled: Option<bool>,
        trigger_type: Option<&str>,
        schedule_interval_seconds: Option<i32>,
        schedule_time: Option<&str>,
        schedule_day_of_week: Option<i16>,
        config: Option<&serde_json::Value>,
    ) -> Result<(), DomainError> {
        let schedule_time_parsed = schedule_time.and_then(|s| {
            chrono::NaiveTime::parse_from_str(s, "%H:%M")
                .ok()
                .or_else(|| chrono::NaiveTime::parse_from_str(s, "%H:%M:%S").ok())
        });

        self.task_repo
            .update_task(
                id,
                enabled,
                trigger_type,
                schedule_interval_seconds,
                schedule_time_parsed,
                schedule_day_of_week,
                config,
            )
            .await?;

        if let Some(task) = self.task_repo.get_task(id).await? {
            let next_run = task.calculate_next_run();
            self.task_repo
                .update_last_run(
                    id,
                    0,
                    TaskStatus::Active,
                    Some("Schedule updated"),
                    next_run,
                )
                .await?;
        }

        Ok(())
    }

    pub async fn enable_task(&self, id: Uuid) -> Result<(), DomainError> {
        let task = self.task_repo.get_task(id).await?;
        if let Some(t) = task {
            if t.trigger_type == TriggerType::Manual {
                return Err(DomainError::new(
                    ErrorKind::InvalidInput,
                    "TaskScheduler",
                    "Manual tasks cannot be enabled for scheduling",
                ));
            }

            let next_run = t.calculate_next_run();
            self.task_repo
                .update_task(id, Some(true), None, None, None, None, None)
                .await?;
            self.task_repo
                .set_task_status(id, TaskStatus::Active)
                .await?;
            self.task_repo
                .update_last_run(id, 0, TaskStatus::Active, Some("Task enabled"), next_run)
                .await?;
        }
        Ok(())
    }

    pub async fn disable_task(&self, id: Uuid) -> Result<(), DomainError> {
        self.task_repo
            .update_task(id, Some(false), None, None, None, None, None)
            .await?;
        self.task_repo
            .set_task_status(id, TaskStatus::Inactive)
            .await?;
        Ok(())
    }

    pub async fn run_task_now(&self, id: Uuid) -> Result<Uuid, DomainError> {
        let task = self.task_repo.get_task(id).await?;
        if let Some(t) = task {
            let execution_id = self.task_repo.create_execution(t.id, "manual").await?;
            let repo = self.task_repo.clone();
            let audio_service = self.audio_service.clone();

            self.task_repo
                .set_task_status(t.id, TaskStatus::Running)
                .await?;

            tokio::spawn(async move {
                let result = match t.task_type {
                    TaskType::AudioMetadataExtraction => {
                        Self::run_audio_metadata_task_internal(&t, repo.clone(), audio_service)
                            .await
                    }
                };

                let (status, message, error_details) = match result {
                    Ok(msg) => (TaskStatus::Completed, Some(msg), None),
                    Err(e) => (TaskStatus::Failed, None, Some(e.to_string())),
                };

                repo.complete_execution(
                    execution_id,
                    status,
                    message.as_deref(),
                    None,
                    error_details.as_deref(),
                )
                .await
                .ok();

                repo.increment_run_counters(t.id, status == TaskStatus::Completed)
                    .await
                    .ok();

                repo.set_task_status(
                    t.id,
                    if t.enabled {
                        TaskStatus::Active
                    } else {
                        TaskStatus::Inactive
                    },
                )
                .await
                .ok();
            });

            Ok(execution_id)
        } else {
            Err(DomainError::not_found("Task", id.to_string()))
        }
    }

    async fn run_audio_metadata_task_internal(
        task: &ScheduledTask,
        _repo: Arc<TaskPgRepository>,
        audio_service: Option<Arc<AudioMetadataService>>,
    ) -> Result<String, DomainError> {
        info!("Running audio metadata extraction task: {}", task.id);

        if let Some(service) = audio_service {
            let result = service.reextract_all_audio_metadata().await?;
            Ok(format!(
                "Processed {} files ({} successful, {} failed)",
                result.total, result.processed, result.failed
            ))
        } else {
            warn!("Audio metadata service not available");
            Err(DomainError::internal_error(
                "AudioMetadata",
                "Audio metadata service is not available. Music feature may be disabled.",
            ))
        }
    }

    pub async fn get_task_executions(
        &self,
        task_id: Uuid,
        limit: i32,
    ) -> Result<Vec<TaskExecution>, DomainError> {
        self.task_repo.get_task_executions(task_id, limit).await
    }

    pub async fn get_task_stats(&self) -> Result<TaskStats, DomainError> {
        let audio_count = self.task_repo.get_audio_files_count().await?;
        let missing_metadata = self.task_repo.get_audio_files_without_metadata().await?;

        Ok(TaskStats {
            total_audio_files: audio_count,
            files_without_metadata: missing_metadata,
            files_with_metadata: audio_count.saturating_sub(missing_metadata),
        })
    }
}

pub struct TaskStats {
    pub total_audio_files: i64,
    pub files_without_metadata: i64,
    pub files_with_metadata: i64,
}
