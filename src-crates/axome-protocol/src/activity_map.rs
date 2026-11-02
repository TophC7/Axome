use serde::{Deserialize, Serialize};

/// Activity from a single harness in a discovered project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredHarnessActivity {
    /// Harness identifier.
    pub harness_id: String,
    /// ISO 8601 timestamp of most recent activity from this harness.
    pub last_active: String,
    /// Per-day session counts for the trailing 7 days.
    /// Index 0 = 6 days ago, index 6 = today.
    pub daily_counts: [u32; 7],
}

/// A project folder discovered from external agent conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProject {
    /// Absolute path to the project directory.
    pub path: String,
    /// Basename of the path (for display).
    pub name: String,
    /// Whether the path exists on disk (deleted projects get greyed out).
    pub path_exists: bool,
    /// ISO 8601 timestamp of most recent activity across all harnesses.
    pub last_active: String,
    /// Per-harness activity data.
    pub harnesses: Vec<DiscoveredHarnessActivity>,
}
