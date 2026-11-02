-- Per-folder Nix environment configuration and cached evaluation results.
CREATE TABLE folder_nix_envs (
  folder_path   TEXT PRIMARY KEY,
  nix_file      TEXT NOT NULL,
  status        TEXT NOT NULL DEFAULT 'pending',
  env_json      TEXT,
  error_message TEXT,
  evaluated_at  TEXT,
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

-- App-shell key/value for hot-restore data: workbench tabs, recent
-- folders, and other frontend-owned state.
CREATE TABLE app_state (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Transcripts of window runs whose process exited. A daemon outlives the
-- desktop, so a run can finish while nobody is attached; its output and exit
-- code wait here instead of in memory.
CREATE TABLE completed_runs (
  run_id       TEXT PRIMARY KEY,
  exit_code    INTEGER,
  output_start INTEGER NOT NULL,
  output       BLOB NOT NULL,
  events_json  TEXT NOT NULL,
  finished_at  TEXT NOT NULL
);

CREATE INDEX completed_runs_finished_at ON completed_runs (finished_at);

CREATE TABLE session_index_sources (
  scope_folder TEXT NOT NULL,   -- canonical folder; '' = all folders
  source_id    TEXT NOT NULL,   -- 'harness:<harness id>' | 'axo'
  preset       TEXT NOT NULL,   -- '' = unnamed launch (always '' for axo)
  refreshed_at TEXT,
  status       TEXT NOT NULL,   -- ready | missing | error
  message      TEXT,
  PRIMARY KEY (scope_folder, source_id, preset)
);

CREATE TABLE indexed_sessions (
  scope_folder      TEXT NOT NULL,
  source_id         TEXT NOT NULL,
  preset            TEXT NOT NULL,
  session_id        TEXT NOT NULL,
  folder_path       TEXT NOT NULL,
  title             TEXT,
  updated_at        TEXT,
  model             TEXT,
  git_branch        TEXT,
  last_message      TEXT,
  last_message_role TEXT CHECK (last_message_role IN ('user', 'agent')),
  message_count     INTEGER CHECK (message_count >= 0 AND message_count <= 4294967295),
  PRIMARY KEY (scope_folder, source_id, preset, session_id),
  FOREIGN KEY (scope_folder, source_id, preset)
    REFERENCES session_index_sources (scope_folder, source_id, preset) ON DELETE CASCADE
);
