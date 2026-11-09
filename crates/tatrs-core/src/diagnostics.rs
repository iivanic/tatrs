//! Structured diagnostics shared by the parser, the loader, and `tatrs check`.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Stable diagnostic codes. Documented in `docs/FORMAT.md`.
pub mod codes {
    // Parse-time, per task.
    pub const INVALID_TITLE: &str = "E0101";
    pub const INVALID_STATUS: &str = "E0102";
    pub const INVALID_PRIORITY: &str = "W0103";
    pub const DUPLICATE_PROPERTY: &str = "W0101";
    pub const MALFORMED_PROPERTY: &str = "W0102";
    pub const INVALID_UTF8: &str = "E0104";
    pub const READ_ERROR: &str = "E0106";
    // Load-time, per directory.
    pub const ORPHAN_TASK_DIR: &str = "E0201";
    pub const INVALID_TASK_ID: &str = "E0202";
    pub const STRAY_ENTRY: &str = "W0203";
    pub const LEFT_OVER_TEMP: &str = "W0204";
    pub const READ_TASKS_DIR: &str = "E0205";
    // Lint-time, per task.
    pub const MISSING_ACCEPTANCE: &str = "W0301";
    pub const TAG_WITH_BRACKETS: &str = "W0302";
    pub const DUPLICATE_TAG: &str = "W0303";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub path: PathBuf,
    pub line: Option<usize>,
    pub message: String,
}

impl Diagnostic {
    pub fn new(
        severity: Severity,
        code: &'static str,
        path: impl Into<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code,
            path: path.into(),
            line,
            message: message.into(),
        }
    }

    pub fn error(
        code: &'static str,
        path: impl Into<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Error, code, path, line, message)
    }

    pub fn warning(
        code: &'static str,
        path: impl Into<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Warning, code, path, line, message)
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// `path:line: error[E0101]: message`, matching the compiler-style output
    /// that `tatr` users already pipe into Emacs and editors.
    pub fn render(&self, path: &Path) -> String {
        let mut out = path.display().to_string();
        if let Some(line) = self.line {
            out.push(':');
            out.push_str(&line.to_string());
        }
        out.push_str(": ");
        out.push_str(self.severity.as_str());
        out.push('[');
        out.push_str(self.code);
        out.push_str("]: ");
        out.push_str(&self.message);
        out
    }

    pub fn to_json(&self, path: &Path) -> serde_json::Value {
        serde_json::json!({
            "severity": self.severity,
            "code": self.code,
            "path": path_to_json(path),
            "line": self.line,
            "message": self.message,
        })
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(&self.path))
    }
}

/// JSON paths are always forward-slash separated so output is stable across
/// platforms.
pub fn path_to_json(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
