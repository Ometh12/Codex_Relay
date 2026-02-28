use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRecord {
    pub id: String,
    pub created_at: String,
    pub op: String,
    pub name: String,
    pub note: Option<String>,
    // Comma-separated tags, e.g. "mac, win, bugfix".
    pub tags: Option<String>,
    pub favorite: bool,
    pub updated_at: Option<String>,
    pub session_id_old: Option<String>,
    pub session_id_new: Option<String>,
    pub effective_session_id: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub vault_dir: String,
    pub bundle_path: String,
    pub vault_rollout_rel_path: Option<String>,
    pub rollout_sha256: Option<String>,
    pub rollout_size: Option<i64>,
    pub local_rollout_path: Option<String>,
}
