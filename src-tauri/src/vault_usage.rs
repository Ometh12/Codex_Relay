use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::db;

#[derive(Debug, Clone, Serialize)]
pub struct VaultUsageItem {
    pub id: String,
    pub created_at: String,
    pub op: String,
    pub name: String,
    pub effective_session_id: Option<String>,
    pub status: String,
    pub vault_dir: String,
    pub bytes: i64,
    pub files: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultUsage {
    pub total_bytes: i64,
    pub total_files: i64,
    pub items: Vec<VaultUsageItem>,
}

pub fn vault_usage_command<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    limit: usize,
) -> Result<VaultUsage, String> {
    db::with_conn(app, |conn| {
        let records = db::transfers_list(conn, limit)?;
        let mut total: i64 = 0;
        let mut total_files: i64 = 0;
        let mut items: Vec<VaultUsageItem> = Vec::new();

        for r in records {
            let dir = PathBuf::from(&r.vault_dir);
            let (bytes, files) = dir_usage(&dir).unwrap_or((0, 0));
            total = total.saturating_add(bytes);
            total_files = total_files.saturating_add(files);
            items.push(VaultUsageItem {
                id: r.id,
                created_at: r.created_at,
                op: r.op,
                name: r.name,
                effective_session_id: r.effective_session_id,
                status: r.status,
                vault_dir: dir.to_string_lossy().to_string(),
                bytes,
                files,
            });
        }

        Ok(VaultUsage {
            total_bytes: total,
            total_files,
            items,
        })
    })
}

fn dir_usage(dir: &Path) -> Result<(i64, i64), String> {
    if !dir.exists() {
        return Ok((0, 0));
    }

    let mut bytes: i64 = 0;
    let mut files: i64 = 0;
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];

    while let Some(d) = stack.pop() {
        let rd = fs::read_dir(&d).map_err(|e| format!("read_dir: {e}"))?;
        for entry in rd {
            let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
            let meta = entry.metadata().map_err(|e| format!("metadata: {e}"))?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                bytes = bytes.saturating_add(i64::try_from(meta.len()).unwrap_or(i64::MAX));
                files = files.saturating_add(1);
            }
        }
    }

    Ok((bytes, files))
}
