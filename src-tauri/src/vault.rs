use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::app_paths;
use sha2::{Digest, Sha256};

pub fn ensure_vault_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app_paths::vault_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create vault dir: {e}"))?;
    Ok(dir)
}

pub fn ensure_transfer_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    transfer_id: &str,
) -> Result<PathBuf, String> {
    let vault = ensure_vault_dir(app)?;
    let dir = vault.join(transfer_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create transfer dir: {e}"))?;
    Ok(dir)
}

pub fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create dir: {e}"))?;
    }
    std::fs::copy(src, dst).map_err(|e| format!("failed to copy file: {e}"))?;
    Ok(())
}

pub fn copy_file_with_sha256(src: &Path, dst: &Path) -> Result<(String, i64), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create dir: {e}"))?;
    }

    // Stream copy while hashing to avoid reading huge rollouts twice (copy + hash).
    let mut r = fs::File::open(src).map_err(|e| format!("failed to open src file: {e}"))?;
    let mut w = fs::File::create(dst).map_err(|e| format!("failed to create dst file: {e}"))?;

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 64];
    let mut total: u64 = 0;

    loop {
        let n = r
            .read(&mut buf)
            .map_err(|e| format!("failed to read src file: {e}"))?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n])
            .map_err(|e| format!("failed to write dst file: {e}"))?;
        hasher.update(&buf[..n]);
        total = total.saturating_add(n as u64);
    }
    w.flush()
        .map_err(|e| format!("failed to flush dst file: {e}"))?;

    let size = i64::try_from(total).map_err(|_| "file too large (exceeds i64)".to_string())?;
    Ok((hex::encode(hasher.finalize()), size))
}

pub fn safe_remove_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(dir).map_err(|e| format!("failed to remove dir: {e}"))?;
    Ok(())
}

pub fn validate_dir_within_vault<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    dir: &Path,
) -> Result<PathBuf, String> {
    // Guardrail: never allow deleting/restoring from a directory outside the vault root,
    // even if the SQLite db is corrupted or manually edited.
    let vault = ensure_vault_dir(app)?;
    let vault = fs::canonicalize(&vault).map_err(|e| format!("canonicalize vault dir: {e}"))?;
    let dir = fs::canonicalize(dir).map_err(|e| format!("canonicalize dir: {e}"))?;
    if !dir.starts_with(&vault) {
        return Err("路径不在存档库范围内（拒绝操作）".to_string());
    }
    Ok(dir)
}
