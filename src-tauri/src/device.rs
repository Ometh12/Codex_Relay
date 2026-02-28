use serde::{Deserialize, Serialize};

use crate::db;

const KEY_DEVICE_ID: &str = "device_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub os: String,
    pub arch: String,
    pub hostname: Option<String>,
}

pub fn get_or_create_device_id(conn: &rusqlite::Connection) -> Result<String, String> {
    if let Some(v) = db::kv_get(conn, KEY_DEVICE_ID)? {
        return Ok(v);
    }

    let id = uuid::Uuid::now_v7().to_string();
    db::kv_set(conn, KEY_DEVICE_ID, &id)?;
    Ok(id)
}

pub fn current_device_info(conn: &rusqlite::Connection) -> Result<DeviceInfo, String> {
    let device_id = get_or_create_device_id(conn)?;
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let hostname = hostname();

    Ok(DeviceInfo {
        device_id,
        os,
        arch,
        hostname,
    })
}

fn hostname() -> Option<String> {
    // Best-effort, no extra deps.
    if let Ok(v) = std::env::var("HOSTNAME") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }
    if let Ok(v) = std::env::var("COMPUTERNAME") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }
    None
}
