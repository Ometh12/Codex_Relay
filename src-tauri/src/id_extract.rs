use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{BufReader, Read},
};

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractSessionIdsFromFileParams {
    pub path: String,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractSessionIdsResult {
    pub source: String,
    pub scanned_bytes: i64,
    pub truncated: bool,
    pub ids: Vec<String>,
}

pub fn extract_session_ids_from_file(
    params: ExtractSessionIdsFromFileParams,
) -> Result<ExtractSessionIdsResult, String> {
    let path = std::path::PathBuf::from(&params.path);
    if !path.exists() {
        return Err("文件不存在".to_string());
    }
    let meta = fs::metadata(&path).map_err(|e| format!("stat file: {e}"))?;
    if !meta.is_file() {
        return Err("不是文件".to_string());
    }

    // Restrict to common text extensions to reduce accidental misuse.
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "md" && ext != "txt" {
        return Err("仅支持 .md / .txt 文件".to_string());
    }

    let max_bytes = params.max_bytes.unwrap_or(16 * 1024 * 1024); // 16 MiB default
    let file_len = meta.len() as usize;
    let truncated = file_len > max_bytes;

    let f = fs::File::open(&path).map_err(|e| format!("open file: {e}"))?;
    let mut reader = BufReader::new(f);

    // Stream read to avoid loading huge files into memory.
    let mut buf = vec![0u8; 64 * 1024];
    let mut scanned: usize = 0;
    let mut tail: Vec<u8> = Vec::new();

    let mut seen: HashSet<String> = HashSet::new();
    let mut ids: Vec<String> = Vec::new();

    while scanned < max_bytes {
        let to_read = std::cmp::min(buf.len(), max_bytes - scanned);
        let n = reader
            .read(&mut buf[..to_read])
            .map_err(|e| format!("read file: {e}"))?;
        if n == 0 {
            break;
        }
        scanned += n;

        let mut chunk: Vec<u8> = Vec::with_capacity(tail.len() + n);
        chunk.extend_from_slice(&tail);
        chunk.extend_from_slice(&buf[..n]);

        extract_session_ids_from_bytes(&chunk, &mut ids, &mut seen);

        // Keep up to 35 bytes to catch ids spanning chunk boundaries (uuid length is 36).
        if chunk.len() > 35 {
            tail = chunk[chunk.len() - 35..].to_vec();
        } else {
            tail = chunk;
        }
    }

    Ok(ExtractSessionIdsResult {
        source: path.to_string_lossy().to_string(),
        scanned_bytes: scanned as i64,
        truncated,
        ids,
    })
}

fn extract_session_ids_from_bytes(bytes: &[u8], out: &mut Vec<String>, seen: &mut HashSet<String>) {
    // UUID-like session id (uuid v7): xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (36 chars)
    const LEN: usize = 36;
    if bytes.len() < LEN {
        return;
    }

    let mut i: usize = 0;
    while i + LEN <= bytes.len() {
        if !is_uuid_hyphenated_at(bytes, i) {
            i += 1;
            continue;
        }
        let s = match std::str::from_utf8(&bytes[i..i + LEN]) {
            Ok(s) => s,
            Err(_) => {
                i += 1;
                continue;
            }
        };
        let id = s.to_ascii_lowercase();
        if seen.insert(id.clone()) {
            out.push(id);
        }
        // Skip ahead; ids are fixed length and do not overlap meaningfully.
        i += LEN;
    }
}

fn is_uuid_hyphenated_at(bytes: &[u8], i: usize) -> bool {
    const LEN: usize = 36;
    let Some(s) = bytes.get(i..i + LEN) else {
        return false;
    };
    if s[8] != b'-' || s[13] != b'-' || s[18] != b'-' || s[23] != b'-' {
        return false;
    }
    for (idx, &b) in s.iter().enumerate() {
        if matches!(idx, 8 | 13 | 18 | 23) {
            continue;
        }
        if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_from_bytes_finds_ids_in_noise_and_dedupes() {
        let s = b"hello codex resume 019bf3ba-8b3f-7ef1-b1f1-212573c83872, ok\n\
                 path rollout-2026-01-25T13-57-27-019bf3ba-8b3f-7ef1-b1f1-212573c83872.jsonl\n\
                 another 019c0de3-259d-7ec3-90e5-e67ec455390e end";
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        extract_session_ids_from_bytes(s, &mut out, &mut seen);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], "019bf3ba-8b3f-7ef1-b1f1-212573c83872");
        assert_eq!(out[1], "019c0de3-259d-7ec3-90e5-e67ec455390e");
    }
}
