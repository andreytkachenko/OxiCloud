-- ============================================================
-- OxiCloud Scheduled Tasks Schema
-- Tasks for background job scheduling and execution
-- ============================================================

CREATE SCHEMA IF NOT EXISTS tasks;

-- Task types enum
DO $BODY$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_type t
        JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'task_type' AND n.nspname = 'tasks'
    ) THEN
        CREATE TYPE tasks.task_type AS ENUM (
            'audio_metadata_extraction'  -- Extract metadata from MP3/audio files for Music section
        );
    END IF;
END $BODY$;

-- Task status enum
DO $BODY$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_type t
        JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'task_status' AND n.nspname = 'tasks'
    ) THEN
        CREATE TYPE tasks.task_status AS ENUM (
            'active',      -- Task is active and scheduled to run
            'inactive',    -- Task is disabled
            'running',     -- Task is currently executing
            'completed',   -- Task completed successfully
            'failed'       -- Task failed
        );
    END IF;
END $BODY$;

-- Trigger type enum
DO $BODY$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_type t
        JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'trigger_type' AND n.nspname = 'tasks'
    ) THEN
        CREATE TYPE tasks.trigger_type AS ENUM (
            'manual',      -- Run only when triggered manually
            'periodic',    -- Run at regular intervals (e.g., every 24 hours)
            'daily',       -- Run once per day at scheduled time
            'weekly',      -- Run once per week at scheduled time
            'on_upload'    -- Run automatically when files are uploaded
        );
    END IF;
END $BODY$;

-- Rename existing column if it exists (for existing databases)
DO $BODY$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'tasks' AND table_name = 'scheduled_tasks' AND column_name = 'schedule_type'
    ) THEN
        ALTER TABLE tasks.scheduled_tasks RENAME COLUMN schedule_type TO trigger_type;
    END IF;
END $BODY$;

-- Scheduled tasks configuration table
CREATE TABLE IF NOT EXISTS tasks.scheduled_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_type tasks.task_type NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    status tasks.task_status NOT NULL DEFAULT 'inactive',
    trigger_type tasks.trigger_type NOT NULL DEFAULT 'manual',
    schedule_interval_seconds INTEGER,  -- For periodic schedule (minimum 60 seconds)
    schedule_time TIME,                -- For daily/weekly schedule (HH:MM:SS)
    schedule_day_of_week SMALLINT,     -- For weekly schedule (0=Sunday, 1=Monday, etc.)
    last_run_at TIMESTAMP WITH TIME ZONE,
    last_run_duration_secs INTEGER,
    last_run_status tasks.task_status,
    last_run_message TEXT,
    next_run_at TIMESTAMP WITH TIME ZONE,
    total_runs INTEGER NOT NULL DEFAULT 0,
    total_successes INTEGER NOT NULL DEFAULT 0,
    total_failures INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by UUID REFERENCES auth.users(id),
    config JSONB NOT NULL DEFAULT '{}'  -- Task-specific configuration
);

CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_status ON tasks.scheduled_tasks(status);
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_enabled ON tasks.scheduled_tasks(enabled) WHERE enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_next_run ON tasks.scheduled_tasks(next_run_at) WHERE enabled = TRUE;

-- Task execution history
CREATE TABLE IF NOT EXISTS tasks.task_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks.scheduled_tasks(id) ON DELETE CASCADE,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP WITH TIME ZONE,
    duration_secs INTEGER,
    status tasks.task_status NOT NULL DEFAULT 'running',
    message TEXT,
    result JSONB,  -- Task-specific result data (e.g., { "processed": 100, "failed": 2 })
    triggered_by TEXT DEFAULT 'schedule',  -- 'schedule', 'manual', 'api', 'upload'
    error_details TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_executions_task_id ON tasks.task_executions(task_id);
CREATE INDEX IF NOT EXISTS idx_task_executions_started_at ON tasks.task_executions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_executions_status ON tasks.task_executions(task_id, status);

-- Insert default tasks
INSERT INTO tasks.scheduled_tasks (task_type, name, description, enabled, trigger_type, config)
VALUES (
    'audio_metadata_extraction',
    'Audio Metadata Extraction',
    'Extract ID3 tags and metadata from MP3 and audio files for the Music section. Scans all audio files and populates title, artist, album, duration, and other metadata.',
    FALSE,
    'manual',
    '{"mime_types": ["audio/mpeg", "audio/mp3", "audio/ogg", "audio/flac", "audio/wav"]}'
)
ON CONFLICT (task_type) DO NOTHING;

COMMENT ON TABLE tasks.scheduled_tasks IS 'Configuration for scheduled and on-demand background tasks';
COMMENT ON TABLE tasks.task_executions IS 'History of task executions with status and results';