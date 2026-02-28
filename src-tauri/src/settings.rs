use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{codex, db};

const KEY_CODEX_HOME_OVERRIDE: &str = "codex_home_override";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCodexHome {
    pub detected_home: String,
    pub override_home: Option<String>,
    pub effective_home: String,
    pub source: String, // "override" | "env" | "default"
}

pub fn get_codex_home_override(conn: &rusqlite::Connection) -> Result<Option<PathBuf>, String> {
    Ok(db::kv_get(conn, KEY_CODEX_HOME_OVERRIDE)?.map(PathBuf::from))
}

pub fn set_codex_home_override(
    conn: &rusqlite::Connection,
    override_path: Option<&str>,
) -> Result<(), String> {
    match override_path {
        Some(p) if !p.trim().is_empty() => db::kv_set(conn, KEY_CODEX_HOME_OVERRIDE, p),
        _ => {
            // Clear override.
            conn.execute(
                "DELETE FROM kv WHERE key = ?1",
                rusqlite::params![KEY_CODEX_HOME_OVERRIDE],
            )
            .map_err(|e| format!("failed to clear codex_home_override: {e}"))?;
            Ok(())
        }
    }
}

pub fn resolve_codex_home(
    conn: &rusqlite::Connection,
) -> Result<(PathBuf, ResolvedCodexHome), String> {
    let detected = codex::detect_codex_home();
    let detected_home = detected.path.to_string_lossy().to_string();

    if let Some(override_home) = get_codex_home_override(conn)? {
        return Ok((
            override_home.clone(),
            ResolvedCodexHome {
                detected_home,
                override_home: Some(override_home.to_string_lossy().to_string()),
                effective_home: override_home.to_string_lossy().to_string(),
                source: "override".to_string(),
            },
        ));
    }

    Ok((
        detected.path.clone(),
        ResolvedCodexHome {
            detected_home: detected_home.clone(),
            override_home: None,
            effective_home: detected_home,
            source: detected.source,
        },
    ))
}
