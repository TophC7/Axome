DROP TABLE indexed_sessions;
DROP TABLE session_index_sources;
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
  scope_folder TEXT NOT NULL,
  source_id    TEXT NOT NULL,
  preset       TEXT NOT NULL,
  session_id   TEXT NOT NULL,
  folder_path  TEXT NOT NULL,
  title        TEXT,
  updated_at   TEXT,
  model        TEXT,
  git_branch   TEXT,
  PRIMARY KEY (scope_folder, source_id, preset, session_id),
  FOREIGN KEY (scope_folder, source_id, preset)
    REFERENCES session_index_sources (scope_folder, source_id, preset) ON DELETE CASCADE
);
