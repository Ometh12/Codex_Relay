use serde::Serialize;

/// A structured error returned to the frontend via `tauri::command`.
///
/// We keep it intentionally small:
/// - `code`: stable-ish machine-readable category for UI routing / future i18n
/// - `message`: human-readable main error message (Chinese UI by default)
/// - `hint`: optional next-step suggestion
/// - `details`: optional debug payload (not always shown in UI)
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint: None,
            details: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    #[allow(dead_code)]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("VALIDATION", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }

    pub fn integrity(message: impl Into<String>) -> Self {
        Self::new("INTEGRITY", message).with_hint("文件可能损坏：请尝试重新导出或重新传输该文件。")
    }

    #[allow(dead_code)]
    pub fn security(message: impl Into<String>) -> Self {
        Self::new("SECURITY", message)
    }

    #[allow(dead_code)]
    pub fn db(message: impl Into<String>) -> Self {
        Self::new("DB", message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new("IO", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("INTERNAL", message)
            .with_hint("请重试；如果持续出现，请附带日志/截图提交 issue。")
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        AppError::new("UNKNOWN", message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        AppError::new("UNKNOWN", message.to_string())
    }
}
