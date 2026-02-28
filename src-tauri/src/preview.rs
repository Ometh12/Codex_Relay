use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::{app_paths, codex, db, settings};

#[derive(Debug, Clone, Serialize)]
pub struct PreviewMessage {
    pub timestamp: Option<String>,
    pub role: String,
    pub text: String,
    pub content_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RolloutPreview {
    pub kind: String,   // "file" | "bundle"
    pub source: String, // file path or bundle path
    pub session_id: Option<String>,
    // "full" when stats were computed by scanning the whole rollout.jsonl,
    // otherwise "tail_window" (stats reflect the scanned tail window).
    pub stats_scope: String,
    pub messages: Vec<PreviewMessage>,
    // Counts within `stats_scope` (may be larger than `max_messages`).
    pub message_counts: BTreeMap<String, i64>,
    // Counts for the returned `messages` only.
    pub message_counts_preview: BTreeMap<String, i64>,
    pub tool_calls: i64,
    pub tool_call_outputs: i64,
    pub scanned_offset: i64,
    pub scanned_bytes: i64,
    pub max_messages: i64,
    pub max_chars_per_message: i64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewRolloutParams {
    pub path: String,
    pub max_messages: Option<usize>,
    pub max_chars_per_message: Option<usize>,
    pub include_meta: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewBundleParams {
    pub bundle_path: String,
    pub max_messages: Option<usize>,
    pub max_chars_per_message: Option<usize>,
    pub include_meta: Option<bool>,
}

pub fn preview_rollout_command<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    params: PreviewRolloutParams,
) -> Result<RolloutPreview, String> {
    let path = PathBuf::from(&params.path);
    if !path.exists() {
        return Err("文件不存在".to_string());
    }

    // Restrict preview to known safe roots to avoid exposing arbitrary file reads via IPC.
    db::with_conn(app, |conn| {
        let (codex_home, _resolved) = settings::resolve_codex_home(conn)?;
        let app_data_dir = app_paths::app_data_dir(app)?;
        let vault_dir = app_paths::vault_dir(app)?;
        let path = validate_preview_path(&path, &[codex_home, app_data_dir, vault_dir])?;

        preview_rollout_file(
            &path,
            params.max_messages,
            params.max_chars_per_message,
            params.include_meta.unwrap_or(false),
        )
    })
}

pub fn preview_bundle_command<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    params: PreviewBundleParams,
) -> Result<RolloutPreview, String> {
    let bundle_path = PathBuf::from(&params.bundle_path);
    if !bundle_path.exists() {
        return Err("bundle.zip 文件不存在".to_string());
    }
    preview_bundle_zip(
        &bundle_path,
        params.max_messages,
        params.max_chars_per_message,
        params.include_meta.unwrap_or(false),
    )
}

fn validate_preview_path(path: &Path, allowed_roots: &[PathBuf]) -> Result<PathBuf, String> {
    let canon = fs::canonicalize(path).map_err(|e| format!("canonicalize path: {e}"))?;
    for root in allowed_roots {
        if let Ok(root_canon) = fs::canonicalize(root) {
            if canon.starts_with(&root_canon) {
                return Ok(canon);
            }
        }
    }
    Err("预览路径不在允许范围内（CODEX_HOME / 应用数据目录 / 存档库）".to_string())
}

fn preview_rollout_file(
    path: &Path,
    max_messages: Option<usize>,
    max_chars_per_message: Option<usize>,
    include_meta: bool,
) -> Result<RolloutPreview, String> {
    let max_messages = max_messages.unwrap_or(10).clamp(1, 1000);
    let max_chars_per_message = max_chars_per_message.unwrap_or(4000).clamp(200, 20000);

    let file_len = fs::metadata(path)
        .map_err(|e| format!("stat rollout: {e}"))?
        .len() as usize;

    // Session id from the first line (cheap).
    let session_id = codex::read_rollout_session_id(path).ok();

    // For small-ish files, a full scan gives better stats (and is still fast).
    // For very large rollouts, keep preview snappy by scanning only the tail window.
    const FULL_SCAN_MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB
    if file_len <= FULL_SCAN_MAX_BYTES {
        let f = fs::File::open(path).map_err(|e| format!("open rollout: {e}"))?;
        let mut reader = BufReader::new(f);
        let mut line = String::new();

        let mut tool_calls: i64 = 0;
        let mut tool_call_outputs: i64 = 0;
        let mut message_counts: BTreeMap<String, i64> = BTreeMap::new();
        let mut messages: std::collections::VecDeque<PreviewMessage> =
            std::collections::VecDeque::new();
        let mut recent_keys: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut recent_queue: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
        const DEDUP_WINDOW: usize = 2048;

        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .map_err(|e| format!("read rollout: {e}"))?;
            if n == 0 {
                break;
            }
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            match parse_preview_line(l, max_chars_per_message) {
                ParsedPreviewLine::Message(m) => {
                    if !include_meta && !matches!(m.role.as_str(), "user" | "assistant") {
                        continue;
                    }
                    let key = message_dedup_key(&m);
                    if recent_keys.contains(&key) {
                        continue;
                    }
                    recent_keys.insert(key);
                    recent_queue.push_back(key);
                    if recent_queue.len() > DEDUP_WINDOW {
                        if let Some(old) = recent_queue.pop_front() {
                            recent_keys.remove(&old);
                        }
                    }

                    *message_counts.entry(m.role.clone()).or_insert(0) += 1;
                    if messages.len() >= max_messages {
                        messages.pop_front();
                    }
                    messages.push_back(m);
                }
                ParsedPreviewLine::ToolCall => tool_calls += 1,
                ParsedPreviewLine::ToolCallOutput => tool_call_outputs += 1,
                ParsedPreviewLine::Other => {}
            }
        }

        let mut message_counts_preview: BTreeMap<String, i64> = BTreeMap::new();
        for m in &messages {
            *message_counts_preview.entry(m.role.clone()).or_insert(0) += 1;
        }

        return Ok(RolloutPreview {
            kind: "file".to_string(),
            source: path.to_string_lossy().to_string(),
            session_id,
            stats_scope: "full".to_string(),
            messages: messages.into_iter().collect(),
            message_counts,
            message_counts_preview,
            tool_calls,
            tool_call_outputs,
            scanned_offset: 0,
            scanned_bytes: file_len as i64,
            max_messages: max_messages as i64,
            max_chars_per_message: max_chars_per_message as i64,
            warning: None,
        });
    }

    // For large rollouts, avoid allocating huge buffers.
    // Scan backwards from the end until we collect enough messages.
    let mut f = fs::File::open(path).map_err(|e| format!("open rollout: {e}"))?;

    let mut cursor: u64 = file_len as u64;
    let block_size: usize = 2 * 1024 * 1024; // 2 MiB blocks
    let mut scanned_bytes: usize = 0;

    let mut tool_calls: i64 = 0;
    let mut tool_call_outputs: i64 = 0;
    let mut message_counts: BTreeMap<String, i64> = BTreeMap::new();
    // We scan from the tail, so this accumulates newest -> oldest.
    let mut messages_rev: Vec<PreviewMessage> = Vec::new();

    let mut recent_keys: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut recent_queue: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
    const DEDUP_WINDOW: usize = 2048;

    // Partial first line of the already-scanned newer block.
    let mut carry: Vec<u8> = Vec::new();
    const MAX_CARRY_BYTES: usize = 8 * 1024 * 1024; // 8 MiB
    let mut warning: Option<String> = None;

    while cursor > 0 {
        let start = cursor.saturating_sub(block_size as u64);
        let len = (cursor - start) as usize;

        f.seek(SeekFrom::Start(start))
            .map_err(|e| format!("seek rollout: {e}"))?;

        let mut chunk = vec![0u8; len];
        f.read_exact(&mut chunk)
            .map_err(|e| format!("read rollout: {e}"))?;
        scanned_bytes += len;
        cursor = start;

        if carry.len() > MAX_CARRY_BYTES {
            // Extremely long JSONL lines are typically tool outputs. Skip the rest of that
            // line to keep preview responsive and avoid unbounded memory growth.
            carry.clear();
            warning = Some("遇到超长日志行，已跳过部分内容，预览可能不完整。".to_string());
        }

        chunk.extend_from_slice(&carry);
        carry.clear();

        // Drop trailing newlines.
        while matches!(chunk.last(), Some(b) if *b == b'\n' || *b == b'\r') {
            chunk.pop();
        }
        if chunk.is_empty() {
            continue;
        }

        // Walk lines from the end (newest -> oldest).
        let mut end = chunk.len();
        while end > 0 {
            let mut i = end;
            while i > 0 && chunk[i - 1] != b'\n' {
                i -= 1;
            }

            let line = &chunk[i..end];

            // Move to previous line (skip the '\n').
            if i == 0 {
                // No more '\n'. This is the (possibly partial) first line in this chunk.
                if cursor == 0 {
                    // We reached the start of the file, so this line is complete.
                    let line = trim_ascii_bytes(line);
                    if !line.is_empty() {
                        if let Ok(s) = std::str::from_utf8(line) {
                            match parse_preview_line(s, max_chars_per_message) {
                                ParsedPreviewLine::Message(m) => {
                                    if !include_meta
                                        && !matches!(m.role.as_str(), "user" | "assistant")
                                    {
                                        continue;
                                    }
                                    let key = message_dedup_key(&m);
                                    if !recent_keys.contains(&key) {
                                        recent_keys.insert(key);
                                        recent_queue.push_back(key);
                                        if recent_queue.len() > DEDUP_WINDOW {
                                            if let Some(old) = recent_queue.pop_front() {
                                                recent_keys.remove(&old);
                                            }
                                        }
                                        *message_counts.entry(m.role.clone()).or_insert(0) += 1;
                                        if messages_rev.len() < max_messages {
                                            messages_rev.push(m);
                                        }
                                    }
                                }
                                ParsedPreviewLine::ToolCall => tool_calls += 1,
                                ParsedPreviewLine::ToolCallOutput => tool_call_outputs += 1,
                                ParsedPreviewLine::Other => {}
                            }
                        }
                    }
                } else {
                    // This line continues into the next older block.
                    carry = line.to_vec();
                }
                break;
            } else {
                let line = trim_ascii_bytes(line);
                if !line.is_empty() {
                    if let Ok(s) = std::str::from_utf8(line) {
                        match parse_preview_line(s, max_chars_per_message) {
                            ParsedPreviewLine::Message(m) => {
                                if !include_meta && !matches!(m.role.as_str(), "user" | "assistant")
                                {
                                    // Skip meta messages by default.
                                } else {
                                    let key = message_dedup_key(&m);
                                    if recent_keys.contains(&key) {
                                        // Skip adjacent duplicates (e.g. response_item vs event_msg).
                                    } else {
                                        recent_keys.insert(key);
                                        recent_queue.push_back(key);
                                        if recent_queue.len() > DEDUP_WINDOW {
                                            if let Some(old) = recent_queue.pop_front() {
                                                recent_keys.remove(&old);
                                            }
                                        }
                                        *message_counts.entry(m.role.clone()).or_insert(0) += 1;
                                        if messages_rev.len() < max_messages {
                                            messages_rev.push(m);
                                        }
                                    }
                                }
                            }
                            ParsedPreviewLine::ToolCall => tool_calls += 1,
                            ParsedPreviewLine::ToolCallOutput => tool_call_outputs += 1,
                            ParsedPreviewLine::Other => {}
                        }
                    }
                }
                end = i - 1;
            }
        }

        if messages_rev.len() >= max_messages {
            break;
        }
    }

    // We scanned from the tail, so reverse back to chronological order.
    messages_rev.reverse();

    let mut message_counts_preview: BTreeMap<String, i64> = BTreeMap::new();
    for m in &messages_rev {
        *message_counts_preview.entry(m.role.clone()).or_insert(0) += 1;
    }

    let scanned_offset = file_len.saturating_sub(scanned_bytes) as i64;
    let stats_scope = if scanned_offset == 0 {
        "full".to_string()
    } else {
        "tail_window".to_string()
    };

    Ok(RolloutPreview {
        kind: "file".to_string(),
        source: path.to_string_lossy().to_string(),
        session_id,
        stats_scope,
        messages: messages_rev,
        message_counts,
        message_counts_preview,
        tool_calls,
        tool_call_outputs,
        scanned_offset,
        scanned_bytes: scanned_bytes as i64,
        max_messages: max_messages as i64,
        max_chars_per_message: max_chars_per_message as i64,
        warning,
    })
}

fn preview_bundle_zip(
    bundle_path: &Path,
    max_messages: Option<usize>,
    max_chars_per_message: Option<usize>,
    include_meta: bool,
) -> Result<RolloutPreview, String> {
    let max_messages = max_messages.unwrap_or(10).clamp(1, 1000);
    let max_chars_per_message = max_chars_per_message.unwrap_or(4000).clamp(200, 20000);

    let f = fs::File::open(bundle_path).map_err(|e| format!("open bundle zip: {e}"))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("read bundle zip: {e}"))?;

    let session_id_from_manifest = read_manifest_session_id_from_zip(&mut z).ok().flatten();

    let rollout_file = z
        .by_name("rollout.jsonl")
        .map_err(|_| "导出包缺少 rollout.jsonl".to_string())?;

    let mut reader = BufReader::new(rollout_file);
    let mut line = String::new();

    let mut message_counts: BTreeMap<String, i64> = BTreeMap::new();
    let mut tool_calls: i64 = 0;
    let mut tool_call_outputs: i64 = 0;
    let mut messages: std::collections::VecDeque<PreviewMessage> =
        std::collections::VecDeque::new();
    let mut recent_keys: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut recent_queue: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
    const DEDUP_WINDOW: usize = 2048;
    let mut session_id_from_rollout: Option<String> = None;
    let mut scanned_bytes: i64 = 0;

    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read rollout.jsonl from zip: {e}"))?;
        if n == 0 {
            break;
        }
        scanned_bytes += n as i64;
        let l = line.trim();
        if l.is_empty() {
            continue;
        }

        // Try to pick session_id from session_meta early.
        if session_id_from_rollout.is_none() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
                if v.get("type").and_then(|x| x.as_str()) == Some("session_meta") {
                    if let Some(id) = v.pointer("/payload/id").and_then(|x| x.as_str()) {
                        session_id_from_rollout = Some(id.to_string());
                    }
                }
            }
        }

        match parse_preview_line(l, max_chars_per_message) {
            ParsedPreviewLine::Message(m) => {
                if !include_meta && !matches!(m.role.as_str(), "user" | "assistant") {
                    continue;
                }
                let key = message_dedup_key(&m);
                if recent_keys.contains(&key) {
                    continue;
                }
                recent_keys.insert(key);
                recent_queue.push_back(key);
                if recent_queue.len() > DEDUP_WINDOW {
                    if let Some(old) = recent_queue.pop_front() {
                        recent_keys.remove(&old);
                    }
                }
                *message_counts.entry(m.role.clone()).or_insert(0) += 1;
                if messages.len() >= max_messages {
                    messages.pop_front();
                }
                messages.push_back(m);
            }
            ParsedPreviewLine::ToolCall => tool_calls += 1,
            ParsedPreviewLine::ToolCallOutput => tool_call_outputs += 1,
            ParsedPreviewLine::Other => {}
        }
    }

    let mut message_counts_preview: BTreeMap<String, i64> = BTreeMap::new();
    for m in &messages {
        *message_counts_preview.entry(m.role.clone()).or_insert(0) += 1;
    }

    Ok(RolloutPreview {
        kind: "bundle".to_string(),
        source: bundle_path.to_string_lossy().to_string(),
        session_id: session_id_from_manifest.or(session_id_from_rollout),
        stats_scope: "full".to_string(),
        messages: messages.into_iter().collect(),
        message_counts,
        message_counts_preview,
        tool_calls,
        tool_call_outputs,
        scanned_offset: 0,
        scanned_bytes,
        max_messages: max_messages as i64,
        max_chars_per_message: max_chars_per_message as i64,
        warning: None,
    })
}

fn read_manifest_session_id_from_zip(
    z: &mut zip::ZipArchive<fs::File>,
) -> Result<Option<String>, String> {
    let mut manifest_file = match z.by_name("manifest.json") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let mut s = String::new();
    manifest_file
        .read_to_string(&mut s)
        .map_err(|e| format!("read manifest.json from zip: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&s).map_err(|e| format!("parse manifest.json: {e}"))?;
    Ok(v.get("session_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string()))
}

enum ParsedPreviewLine {
    Message(PreviewMessage),
    ToolCall,
    ToolCallOutput,
    Other,
}

fn trim_ascii_bytes(mut s: &[u8]) -> &[u8] {
    while let Some(&b) = s.first() {
        if b.is_ascii_whitespace() {
            s = &s[1..];
        } else {
            break;
        }
    }
    while let Some(&b) = s.last() {
        if b.is_ascii_whitespace() {
            s = &s[..s.len() - 1];
        } else {
            break;
        }
    }
    s
}

fn parse_preview_line(line: &str, max_chars: usize) -> ParsedPreviewLine {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return ParsedPreviewLine::Other,
    };

    let t = v.get("type").and_then(|x| x.as_str());
    match t {
        Some("response_item") => parse_response_item(&v, max_chars),
        Some("event_msg") => parse_event_msg(&v, max_chars),
        _ => ParsedPreviewLine::Other,
    }
}

fn parse_response_item(v: &serde_json::Value, max_chars: usize) -> ParsedPreviewLine {
    let payload = match v.get("payload") {
        Some(p) => p,
        None => return ParsedPreviewLine::Other,
    };
    let payload_type = payload.get("type").and_then(|x| x.as_str());
    match payload_type {
        Some("message") => {
            let role = payload
                .get("role")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .to_string();
            let timestamp = v
                .get("timestamp")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let (text, content_types) = extract_message_text(payload);
            let text = truncate_chars(&text, max_chars);
            ParsedPreviewLine::Message(PreviewMessage {
                timestamp,
                role,
                text,
                content_types,
            })
        }
        Some("function_call") => ParsedPreviewLine::ToolCall,
        Some("function_call_output") => ParsedPreviewLine::ToolCallOutput,
        _ => ParsedPreviewLine::Other,
    }
}

fn parse_event_msg(v: &serde_json::Value, max_chars: usize) -> ParsedPreviewLine {
    let payload = match v.get("payload") {
        Some(p) => p,
        None => return ParsedPreviewLine::Other,
    };
    let payload = match payload.as_object() {
        Some(p) => p,
        None => return ParsedPreviewLine::Other,
    };
    let payload_type = payload
        .get("type")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown");

    // In many Codex rollouts, high-level user/assistant messages are also recorded
    // as event_msg payloads (e.g. user_message / agent_message). Parsing these makes
    // preview more robust across Codex CLI versions.
    let role = match payload_type {
        "user_message" => "user",
        "agent_message" => "assistant",
        _ => return ParsedPreviewLine::Other,
    };

    let message = payload
        .get("message")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    if message.trim().is_empty() {
        return ParsedPreviewLine::Other;
    }

    let timestamp = v
        .get("timestamp")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());

    ParsedPreviewLine::Message(PreviewMessage {
        timestamp,
        role: role.to_string(),
        text: truncate_chars(message, max_chars),
        // Keep a lightweight hint so users can see which parser path was used.
        content_types: vec!["event_msg".to_string(), payload_type.to_string()],
    })
}

fn extract_message_text(payload: &serde_json::Value) -> (String, Vec<String>) {
    let mut parts: Vec<String> = Vec::new();
    let mut content_types: Vec<String> = Vec::new();

    let content = payload.get("content").and_then(|x| x.as_array());
    if let Some(items) = content {
        for item in items {
            let it = item
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown");
            content_types.push(it.to_string());

            match it {
                "input_text" | "output_text" => {
                    if let Some(t) = item.get("text").and_then(|x| x.as_str()) {
                        parts.push(t.to_string());
                    }
                }
                _ => {
                    // Keep a lightweight placeholder for non-text content.
                    parts.push(format!("[{it}]"));
                }
            }
        }
    }

    (parts.join("\n"), content_types)
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, c) in input.chars().enumerate() {
        if i >= max_chars {
            out.push_str("\n…(已截断)");
            break;
        }
        out.push(c);
    }
    out
}

fn message_dedup_key(m: &PreviewMessage) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    m.role.hash(&mut h);
    // Timestamps may differ by a few milliseconds between `event_msg` and `response_item`.
    // Normalize to seconds to dedupe adjacent duplicates reliably.
    let ts = m.timestamp.as_deref().unwrap_or("");
    let ts = if ts.len() >= 19 { &ts[..19] } else { ts };
    ts.hash(&mut h);
    // Avoid hashing a huge string; 400 chars is enough to dedupe adjacent duplicates.
    let snippet: String = m.text.chars().take(400).collect();
    snippet.hash(&mut h);
    h.finish()
}
