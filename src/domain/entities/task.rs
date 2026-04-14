use chrono::{DateTime, Datelike, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    AudioMetadataExtraction,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::AudioMetadataExtraction => "audio_metadata_extraction",
        }
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "audio_metadata_extraction" => Some(TaskType::AudioMetadataExtraction),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerType {
    Manual,
    Periodic,
    Daily,
    Weekly,
    OnUpload,
}

impl TriggerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TriggerType::Manual => "manual",
            TriggerType::Periodic => "periodic",
            TriggerType::Daily => "daily",
            TriggerType::Weekly => "weekly",
            TriggerType::OnUpload => "on_upload",
        }
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(TriggerType::Manual),
            "periodic" => Some(TriggerType::Periodic),
            "daily" => Some(TriggerType::Daily),
            "weekly" => Some(TriggerType::Weekly),
            "on_upload" => Some(TriggerType::OnUpload),
            _ => None,
        }
    }
}

impl FromStr for TriggerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TriggerType::try_from_str(s).ok_or_else(|| format!("Invalid trigger type: {}", s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Active,
    Inactive,
    Running,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Active => "active",
            TaskStatus::Inactive => "inactive",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(TaskStatus::Active),
            "inactive" => Some(TaskStatus::Inactive),
            "running" => Some(TaskStatus::Running),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub task_type: TaskType,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub status: TaskStatus,
    pub trigger_type: TriggerType,
    pub schedule_interval_seconds: Option<i32>,
    pub schedule_time: Option<NaiveTime>,
    pub schedule_day_of_week: Option<i16>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_duration_secs: Option<i32>,
    pub last_run_status: Option<TaskStatus>,
    pub last_run_message: Option<String>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub total_runs: i32,
    pub total_successes: i32,
    pub total_failures: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    pub id: Uuid,
    pub task_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i32>,
    pub status: TaskStatus,
    pub message: Option<String>,
    pub result: Option<serde_json::Value>,
    pub triggered_by: String,
    pub error_details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
}

impl ScheduledTask {
    pub fn is_due(&self) -> bool {
        if !self.enabled {
            return false;
        }

        match self.trigger_type {
            TriggerType::Manual | TriggerType::OnUpload => false,
            TriggerType::Periodic | TriggerType::Daily | TriggerType::Weekly => {
                if let Some(next_run) = self.next_run_at {
                    Utc::now() >= next_run
                } else {
                    true
                }
            }
        }
    }

    pub fn calculate_next_run(&self) -> Option<DateTime<Utc>> {
        match self.trigger_type {
            TriggerType::Manual | TriggerType::OnUpload => None,
            TriggerType::Periodic => {
                let interval = self.schedule_interval_seconds.unwrap_or(86400);
                let now = Utc::now();
                let base = self.last_run_at.unwrap_or(now);
                Some(base + chrono::Duration::seconds(interval as i64))
            }
            TriggerType::Daily => {
                if let Some(time) = self.schedule_time {
                    let now = Utc::now();
                    let today = now.date_naive();
                    let next_date = if let Some(last_run) = self.last_run_at {
                        let last_date = last_run.date_naive();
                        if last_date < today {
                            today
                        } else {
                            today + chrono::Duration::days(1)
                        }
                    } else {
                        today
                    };
                    let naive_dt = next_date.and_time(time);
                    Some(DateTime::from_naive_utc_and_offset(naive_dt, Utc))
                } else {
                    None
                }
            }
            TriggerType::Weekly => {
                if let Some(time) = self.schedule_time {
                    let target_day = self.schedule_day_of_week.unwrap_or(0);
                    let now = Utc::now();
                    let current_weekday = now.weekday().num_days_from_sunday() as i16;
                    let days_until = (target_day - current_weekday + 7) % 7;
                    let next_date = if days_until == 0 {
                        let today_time = now.time();
                        if today_time >= time {
                            now.date_naive() + chrono::Duration::days(7)
                        } else {
                            now.date_naive()
                        }
                    } else {
                        now.date_naive() + chrono::Duration::days(days_until as i64)
                    };
                    let naive_dt = next_date.and_time(time);
                    Some(DateTime::from_naive_utc_and_offset(naive_dt, Utc))
                } else {
                    None
                }
            }
        }
    }
}
