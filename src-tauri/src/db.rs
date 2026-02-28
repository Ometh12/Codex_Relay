use rusqlite::{params, Connection, OptionalExtension};

use crate::{app_paths, transfers::TransferRecord};

fn read_transfer_row(row: &rusqlite::Row) -> Result<TransferRecord, rusqlite::Error> {
    Ok(TransferRecord {
        id: row.get(0)?,
        created_at: row.get(1)?,
        op: row.get(2)?,
        name: row.get(3)?,
        note: row.get(4)?,
        tags: row.get(5)?,
        favorite: row.get(6)?,
        updated_at: row.get(7)?,
        session_id_old: row.get(8)?,
        session_id_new: row.get(9)?,
        effective_session_id: row.get(10)?,
        status: row.get(11)?,
        error_message: row.get(12)?,
        vault_dir: row.get(13)?,
        bundle_path: row.get(14)?,
        vault_rollout_rel_path: row.get(15)?,
        rollout_sha256: row.get(16)?,
        rollout_size: row.get(17)?,
        local_rollout_path: row.get(18)?,
    })
}

fn open<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Connection, String> {
    let db_path = app_paths::db_path(app)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create app data dir: {e}"))?;
    }
    let conn = Connection::open(db_path).map_err(|e| format!("failed to open db: {e}"))?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS kv (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS transfers (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  op TEXT NOT NULL,
  name TEXT NOT NULL,
  note TEXT,
  tags TEXT,
  favorite INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT,
  session_id_old TEXT,
  session_id_new TEXT,
  effective_session_id TEXT,
  status TEXT NOT NULL,
  error_message TEXT,
  vault_dir TEXT NOT NULL,
  bundle_path TEXT NOT NULL,
  vault_rollout_rel_path TEXT,
  rollout_sha256 TEXT,
  rollout_size INTEGER,
  local_rollout_path TEXT
);
"#,
    )
    .map_err(|e| format!("failed to init db: {e}"))?;

    migrate_transfers(conn)?;
    Ok(())
}

fn migrate_transfers(conn: &Connection) -> Result<(), String> {
    // Handle older databases created before we had `tags` / `favorite` / `updated_at`.
    let mut stmt = conn
        .prepare("PRAGMA table_info(transfers)")
        .map_err(|e| format!("failed to read transfers schema: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("failed to read transfers schema rows: {e}"))?;

    let mut cols = std::collections::HashSet::<String>::new();
    for r in rows {
        cols.insert(r.map_err(|e| format!("failed to read transfers schema row: {e}"))?);
    }

    if !cols.contains("tags") {
        conn.execute("ALTER TABLE transfers ADD COLUMN tags TEXT", [])
            .map_err(|e| format!("failed to add transfers.tags: {e}"))?;
    }
    if !cols.contains("favorite") {
        conn.execute(
            "ALTER TABLE transfers ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("failed to add transfers.favorite: {e}"))?;
    }
    if !cols.contains("updated_at") {
        conn.execute("ALTER TABLE transfers ADD COLUMN updated_at TEXT", [])
            .map_err(|e| format!("failed to add transfers.updated_at: {e}"))?;
    }
    Ok(())
}

pub fn with_conn<R: tauri::Runtime, T, E>(
    app: &tauri::AppHandle<R>,
    f: impl FnOnce(&Connection) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<String>,
{
    let conn = open(app).map_err(E::from)?;
    init(&conn).map_err(E::from)?;
    f(&conn)
}

pub fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let v = conn
        .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(|e| format!("failed to read kv: {e}"))?;
    Ok(v)
}

pub fn kv_set(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO kv(key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| format!("failed to write kv: {e}"))?;
    Ok(())
}

pub fn transfers_insert(conn: &Connection, r: &TransferRecord) -> Result<(), String> {
    conn.execute(
        r#"
INSERT INTO transfers (
  id, created_at, op, name, note, session_id_old, session_id_new, effective_session_id,
  status, error_message, vault_dir, bundle_path, vault_rollout_rel_path, rollout_sha256,
  rollout_size, local_rollout_path
) VALUES (
  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
  ?9, ?10, ?11, ?12, ?13, ?14,
  ?15, ?16
)
"#,
        params![
            r.id,
            r.created_at,
            r.op,
            r.name,
            r.note,
            r.session_id_old,
            r.session_id_new,
            r.effective_session_id,
            r.status,
            r.error_message,
            r.vault_dir,
            r.bundle_path,
            r.vault_rollout_rel_path,
            r.rollout_sha256,
            r.rollout_size,
            r.local_rollout_path,
        ],
    )
    .map_err(|e| format!("failed to insert transfer: {e}"))?;
    Ok(())
}

pub fn transfers_list(conn: &Connection, limit: usize) -> Result<Vec<TransferRecord>, String> {
    let mut stmt = conn
        .prepare(
            r#"
SELECT
  id, created_at, op, name, note, tags, favorite, updated_at,
  session_id_old, session_id_new, effective_session_id,
  status, error_message, vault_dir, bundle_path, vault_rollout_rel_path, rollout_sha256,
  rollout_size, local_rollout_path
FROM transfers
ORDER BY created_at DESC
LIMIT ?1
"#,
        )
        .map_err(|e| format!("failed to prepare transfers_list: {e}"))?;

    let rows = stmt
        .query_map(
            params![i64::try_from(limit).unwrap_or(200)],
            read_transfer_row,
        )
        .map_err(|e| format!("failed to query transfers_list: {e}"))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("failed to read transfers_list row: {e}"))?);
    }
    Ok(out)
}

pub fn transfers_get(conn: &Connection, id: &str) -> Result<Option<TransferRecord>, String> {
    conn.query_row(
        r#"
SELECT
  id, created_at, op, name, note, tags, favorite, updated_at,
  session_id_old, session_id_new, effective_session_id,
  status, error_message, vault_dir, bundle_path, vault_rollout_rel_path, rollout_sha256,
  rollout_size, local_rollout_path
FROM transfers
WHERE id = ?1
"#,
        params![id],
        read_transfer_row,
    )
    .optional()
    .map_err(|e| format!("failed to query transfer: {e}"))
}

pub fn transfers_latest_for_sessions(
    conn: &Connection,
    session_ids: &[String],
) -> Result<Vec<TransferRecord>, String> {
    if session_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (0..session_ids.len())
        .map(|i| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        r#"
SELECT
  id, created_at, op, name, note, tags, favorite, updated_at,
  session_id_old, session_id_new, effective_session_id,
  status, error_message, vault_dir, bundle_path, vault_rollout_rel_path, rollout_sha256,
  rollout_size, local_rollout_path
FROM transfers
WHERE effective_session_id IN ({placeholders})
ORDER BY created_at DESC
"#
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("failed to prepare transfers_latest_for_sessions: {e}"))?;

    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(session_ids.iter()),
            read_transfer_row,
        )
        .map_err(|e| format!("failed to query transfers_latest_for_sessions: {e}"))?;

    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::new();
    for r in rows {
        let r = r.map_err(|e| format!("failed to read transfers_latest_for_sessions row: {e}"))?;
        let sid = r.effective_session_id.clone().unwrap_or_default();
        if sid.is_empty() {
            continue;
        }
        if seen.contains(&sid) {
            continue;
        }
        seen.insert(sid);
        out.push(r);
    }
    Ok(out)
}

pub fn transfers_update_meta(
    conn: &Connection,
    id: &str,
    name: &str,
    note: Option<&str>,
    tags: Option<&str>,
    favorite: bool,
    updated_at: &str,
) -> Result<TransferRecord, String> {
    let changed = conn
        .execute(
            r#"
UPDATE transfers
SET name = ?2,
    note = ?3,
    tags = ?4,
    favorite = ?5,
    updated_at = ?6
WHERE id = ?1
"#,
            params![id, name, note, tags, favorite, updated_at],
        )
        .map_err(|e| format!("failed to update transfer: {e}"))?;
    if changed == 0 {
        return Err("未找到历史记录".to_string());
    }
    transfers_get(conn, id)?.ok_or_else(|| "未找到历史记录".to_string())
}

pub fn transfers_delete(conn: &Connection, id: &str) -> Result<(), String> {
    let changed = conn
        .execute("DELETE FROM transfers WHERE id = ?1", params![id])
        .map_err(|e| format!("failed to delete transfer: {e}"))?;
    if changed == 0 {
        return Err("未找到历史记录".to_string());
    }
    Ok(())
}
