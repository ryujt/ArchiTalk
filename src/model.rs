use crate::config::Settings;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn hash(text: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(text.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub workspace_revision: u64,
    pub settings_revision: u64,
    #[serde(default)]
    pub settings_overridden: bool,
    pub settings: Settings,
    pub sessions: Vec<Session>,
    pub runs: Vec<Run>,
    pub exports: Vec<ExportEntry>,
}
impl Workspace {
    pub fn new(settings: Settings) -> Self {
        Self {
            workspace_revision: 0,
            settings_revision: 0,
            settings_overridden: false,
            settings,
            sessions: vec![],
            runs: vec![],
            exports: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub revision: u64,
    pub design_revision: u64,
    pub created_at: String,
    pub messages: Vec<Message>,
    pub evidence: Vec<Evidence>,
    pub claims: Vec<Claim>,
    pub requirements: Vec<Requirement>,
    pub decisions: Vec<Decision>,
    pub risks: Vec<Risk>,
    pub coverage: Vec<Coverage>,
    pub questions: Vec<Question>,
    pub drafts: Vec<Draft>,
    pub findings: Vec<Finding>,
    pub content_status: String,
    pub active_run_id: Option<String>,
    pub last_question_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub path: String,
    pub hash: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub kind: String,
    pub status: String,
    pub evidence_refs: Vec<String>,
    pub source_quote: String,
    #[serde(default)]
    pub supersedes: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    pub acceptance: String,
    pub evidence_refs: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub source_quote: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub alternatives: Vec<String>,
    pub rationale: String,
    pub consequences: String,
    pub status: String,
    pub requirement_refs: Vec<String>,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub source_quote: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    pub id: String,
    pub text: String,
    pub impact: String,
    pub action: String,
    pub blocker: bool,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coverage {
    pub id: String,
    pub title: String,
    pub required: bool,
    pub blocker: bool,
    pub status: String,
    pub assessment_source: String,
    pub assessment_status: String,
    pub reason: String,
    pub evidence_refs: Vec<String>,
    pub owner: String,
    pub follow_up: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub target_ref: String,
    pub text: String,
    pub reason: String,
    pub options: Vec<String>,
    pub priority_class: u8,
    pub depends_on: Vec<String>,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    pub input_revision: u64,
    pub markdown: String,
    #[serde(default)]
    pub documents: Vec<crate::design_package::DesignDocument>,
    pub hash: String,
    pub created_at: String,
    pub content_status: String,
    pub reviewed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub draft_id: String,
    pub severity: String,
    pub text: String,
    pub evidence: String,
    pub impact: String,
    pub proposed_resolution: String,
    pub disposition: Option<String>,
    pub disposition_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub status: String,
    pub input_revision: u64,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub effective_settings: Value,
    pub error: Option<RunError>,
    pub process_termination: String,
    pub progress: String,
    pub resolved_model: Option<String>,
    pub cli_version: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunError {
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEntry {
    pub id: String,
    pub session_id: String,
    pub document_revision: u64,
    pub document_hash: String,
    pub draft_id: String,
    #[serde(default)]
    pub preview_revision: u64,
    #[serde(default)]
    pub source_document_hash: String,
    pub status: String,
    pub plan: Value,
    pub result: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub command_id: String,
    pub expected_revision: u64,
    pub payload: Value,
}
