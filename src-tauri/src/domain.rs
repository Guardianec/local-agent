use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub type Result<T> = std::result::Result<T, AppError>;
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
    pub fn detail(mut self, detail: impl ToString) -> Self {
        self.details = Some(detail.to_string());
        self
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new(
            "FILE_ACCESS",
            "A file could not be accessed. Your existing files have been preserved.",
        )
        .detail(e)
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::new("STORAGE", "Local task storage could not be updated.").detail(e)
    }
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(
            "INVALID_DATA",
            "The data did not match the expected format.",
        )
        .detail(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/generated/")]
pub enum Mode {
    Observe,
    #[default]
    Assisted,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct RootIdentity {
    pub volume_id: String,
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct GitInfo {
    pub head: Option<String>,
    pub branch: String,
    pub index_hash: Option<String>,
    pub status: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub adapter: String,
    pub script: String,
    pub description: String,
    pub config_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Project {
    #[serde(default)]
    pub browser_only: bool,
    pub id: String,
    pub name: String,
    pub path: String,
    pub identity: RootIdentity,
    pub stack: String,
    pub mode: Mode,
    pub revision: u32,
    pub opened_at: String,
    pub files: Vec<String>,
    pub exclusions: Vec<String>,
    pub git: Option<GitInfo>,
    pub recipes: Vec<Recipe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Route {
    pub level: u8,
    pub score: u32,
    pub strategy: String,
    pub reasons: Vec<String>,
    pub estimated_files: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Task {
    #[serde(default)]
    pub intent: Intent,
    #[serde(default)]
    pub intent_reason: String,
    #[serde(default)]
    pub conversation_context: String,
    #[serde(default)]
    pub result: Option<TaskResult>,
    pub id: String,
    pub project_id: String,
    pub request: String,
    pub preset: String,
    pub state: String,
    pub change_state: String,
    pub revision: u32,
    pub mode: Mode,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub model: Option<String>,
    pub route: Route,
    pub summary: String,
    pub warnings: Vec<String>,
    pub events: Vec<AgentEvent>,
    pub checks: Vec<Check>,
    pub approval: Option<Approval>,
    pub turns: u32,
    pub tool_calls: u32,
    pub commands: u32,
    pub generated_tokens: u32,
    #[serde(default)]
    pub model_eval_ms: f64,
    pub parent_task_id: Option<String>,
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub inputs: Vec<ContentBlock>,
    #[serde(default = "enabled")]
    pub automatic_escalation: bool,
    #[serde(default)]
    pub stages: Vec<ModelStage>,
    #[serde(default)]
    pub continuation: Option<TaskContinuationContext>,
    #[serde(default)]
    pub metrics: TaskMetrics,
}
impl Task {
    pub fn transition(&mut self, next: &str) {
        if self.state != next {
            let legal = crate::interaction::legal_transition(&self.state, next)
                || (self.finished_at.is_none() && next == "cancelled");
            self.event(
                if legal { "state" } else { "diagnostic" },
                if legal {
                    "Task state changed"
                } else {
                    "Unexpected state transition"
                },
                format!("{} → {}", self.state, next),
            );
            self.state = next.into();
        }
    }
    pub fn active(&self) -> bool {
        matches!(
            self.state.as_str(),
            "queued"
                | "preparing"
                | "routing"
                | "running"
                | "waiting_approval"
                | "validating"
                | "stopping"
        )
    }
    pub fn event(&mut self, kind: &str, title: impl Into<String>, detail: impl Into<String>) {
        self.events.push(AgentEvent {
            id: id(),
            seq: self.events.last().map_or(1, |e| e.seq + 1),
            at: now(),
            kind: kind.into(),
            title: crate::security::bounded(&crate::security::redact(&title.into()), 256),
            detail: crate::security::bounded(&crate::security::redact(&detail.into()), 24000),
        });
        if self.events.len() > 200 {
            self.events.remove(0);
        }
        self.revision += 1;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/generated/")]
pub enum Intent {
    Converse,
    Inspect,
    #[default]
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct TaskFailure {
    pub category: String,
    pub explanation: String,
    pub recoverable: bool,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct TaskResult {
    pub task_id: String,
    pub status: String,
    pub intent: Intent,
    pub user_message: String,
    pub files_changed: Vec<String>,
    pub validations: Vec<String>,
    pub warnings: Vec<String>,
    pub failure: Option<TaskFailure>,
    pub suggested_next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct AgentEvent {
    pub id: String,
    pub seq: u32,
    pub at: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Approval {
    pub id: String,
    pub task_id: String,
    pub operation_hash: String,
    pub title: String,
    pub detail: String,
    pub preview: String,
    pub risk: String,
    pub expires_at: String,
    pub allow_task: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Check {
    pub id: String,
    pub recipe_id: String,
    pub name: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub output: String,
    pub manifest_hash: String,
    pub duration_ms: f64,
    pub passed: Option<u32>,
    pub failed: Option<u32>,
    #[serde(default)]
    pub outcome_status: Option<String>,
}
impl Check {
    pub fn outcome(&self) -> &str {
        self.outcome_status.as_deref().unwrap_or_else(|| {
            if self.status == "stale" {
                match self.exit_code {
                    Some(0) => "passed",
                    Some(_) => "failed",
                    None => "unknown",
                }
            } else {
                &self.status
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct FileDelta {
    pub path: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub before: String,
    pub after: String,
    pub additions: u32,
    pub deletions: u32,
    pub kind: String,
    pub eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Review {
    pub task_id: String,
    pub revision: u32,
    pub files: Vec<FileDelta>,
    pub warnings: Vec<String>,
    pub can_apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ModelInfo {
    pub name: String,
    pub digest: String,
    pub size: f64,
    pub parameter_size: String,
    pub quantization: String,
    pub loaded: bool,
    pub vram: Option<f64>,
    pub loaded_bytes: Option<f64>,
    pub capability: String,
    pub local: bool,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub thinking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct RuntimeInfo {
    pub status: String,
    pub version: Option<String>,
    pub installed: bool,
    pub models: Vec<ModelInfo>,
    pub message: String,
    pub gpu: Option<GpuInfo>,
    pub calibrations: Vec<ModelCalibration>,
    #[serde(default)]
    pub memory: Option<MemoryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct GpuInfo {
    pub name: String,
    pub total_bytes: f64,
    pub free_bytes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ModelCalibration {
    pub model: String,
    pub digest: String,
    pub runtime: String,
    pub context_size: u32,
    pub passed: bool,
    pub full_gpu: Option<bool>,
    pub elapsed_ms: f64,
    pub tokens_per_second: Option<f64>,
    pub allocation_bytes: Option<f64>,
    pub free_gpu_bytes: Option<f64>,
    pub message: String,
    pub evidence: String,
    pub tested_at: String,
    #[serde(default)]
    pub peak_ram_bytes: Option<f64>,
    #[serde(default)]
    pub minimum_free_ram_bytes: Option<f64>,
    #[serde(default)]
    pub time_to_first_token_ms: Option<f64>,
    #[serde(default)]
    pub retries: u32,
    #[serde(default)]
    pub escalations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ModelProfiles {
    pub fast: Option<String>,
    pub balanced: Option<String>,
    pub strong: Option<String>,
    pub strong_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Settings {
    pub theme: String,
    pub model: Option<String>,
    #[serde(default)]
    pub model_profiles: ModelProfiles,
    pub context_size: u32,
    pub node_path: Option<String>,
    pub python_path: Option<String>,
    pub git_path: Option<String>,
    pub last_project_id: Option<String>,
    #[serde(default)]
    pub intelligence: IntelligenceSettings,
}

pub fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct IntelligenceSettings {
    pub registry_version: u32,
    pub automatic_escalation: bool,
    pub fast_context: u32,
    pub balanced_context: u32,
    pub strong_context: u32,
}
impl Default for IntelligenceSettings {
    fn default() -> Self {
        Self {
            registry_version: 0,
            automatic_escalation: true,
            fast_context: 16384,
            balanced_context: 32768,
            strong_context: 32768,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct MemoryInfo {
    pub total_bytes: f64,
    pub available_bytes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ModelInstallProgress {
    pub model: String,
    pub status: String,
    pub completed: f64,
    pub total: Option<f64>,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct CleanupCandidate {
    pub name: String,
    pub owned: bool,
    pub size: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../src/generated/")]
pub enum ContentBlock {
    Text { text: String },
    Image { attachment_id: String },
    FileReference { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ImageAttachment {
    pub id: String,
    pub name: String,
    pub blob: String,
    pub original_blob: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u32,
    pub resized: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct TaskMetrics {
    pub time_to_first_token_ms: Option<f64>,
    pub inference_ms: f64,
    pub peak_ram_bytes: Option<f64>,
    pub peak_vram_bytes: Option<f64>,
    pub retries: u32,
    pub escalations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ModelStage {
    pub tier: String,
    pub model: String,
    pub context: u32,
    pub started_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct TaskContinuationContext {
    pub task_id: String,
    pub original_request: String,
    pub project_summary: String,
    pub constraints: Vec<String>,
    pub relevant_files: Vec<String>,
    pub tool_outcomes: Vec<String>,
    pub checks: Vec<String>,
    pub current_diff: String,
    pub snapshot_id: Option<String>,
    pub accepted_permissions: Vec<String>,
    pub rejected_permissions: Vec<String>,
    pub observations: Vec<String>,
    pub unresolved: Vec<String>,
    pub previous_model: Option<String>,
    pub escalation_reason: String,
    pub images: Vec<String>,
    pub used_turns: u32,
    pub used_tools: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct Bootstrap {
    pub schema_version: u8,
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub settings: Settings,
    pub storage_path: String,
    pub browser_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct HandoffPreview {
    pub id: String,
    pub task_id: String,
    pub markdown: String,
    pub patch: Option<String>,
    pub omissions: Vec<String>,
    pub bytes: u32,
}
