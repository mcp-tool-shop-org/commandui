use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    SessionNotFound,
    SessionDisconnected,
    ExecutionFailed,
    PlannerFailed,
    ValidationFailed,
    DatabaseError,
    /// The id is not in the store, or a suggestion is no longer pending.
    NotFound,
    /// The shell in this session has exited.
    SessionExited,
    NotImplemented,
    UnknownError,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<String>,
}

impl ApiError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ValidationFailed,
            message: message.into(),
            details: None,
        }
    }

    pub fn planner(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::PlannerFailed,
            message: message.into(),
            details: None,
        }
    }

    pub fn execution(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ExecutionFailed,
            message: message.into(),
            details: None,
        }
    }

    pub fn session_not_found(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::SessionNotFound,
            message: message.into(),
            details: None,
        }
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::DatabaseError,
            message: message.into(),
            details: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: message.into(),
            details: None,
        }
    }

    pub fn session_exited(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::SessionExited,
            message: message.into(),
            details: None,
        }
    }

    /// Persistence `Err(String)` values. Missing-id phrases become `NOT_FOUND`.
    /// Every other failure stays `DATABASE_ERROR`. The phrases are the ones
    /// `runtime-persistence` returns; the UI matches the code, not the words.
    pub fn from_persistence(message: impl Into<String>) -> Self {
        let message = message.into();
        if persistence_is_missing(&message) {
            Self::not_found(message)
        } else {
            Self::database(message)
        }
    }

    /// Terminal and session `Err(String)` values. The exited-shell sentence
    /// becomes `SESSION_EXITED`. Every other failure stays `EXECUTION_FAILED`.
    pub fn from_execution(message: impl Into<String>) -> Self {
        let message = message.into();
        if message == "The shell in this session has exited; open a new session" {
            Self::session_exited(message)
        } else {
            Self::execution(message)
        }
    }
}

fn persistence_is_missing(message: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "history update: no history item with id ",
        "workflow delete: no workflow with id ",
        "memory delete: no memory item with id ",
        "suggestion not pending: ",
        "suggestion not found: ",
        "dismiss suggestion: no suggestion with id ",
    ];
    PREFIXES.iter().any(|prefix| message.starts_with(prefix))
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_error(err: ApiError, code: &str, message: &str) {
        assert_eq!(err.to_string(), message);
        let as_error: &dyn std::error::Error = &err;
        assert!(as_error.source().is_none());
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], code);
        assert_eq!(json["message"], message);
    }

    #[test]
    fn constructors_display_and_serialize() {
        assert_error(ApiError::validation("bad input"), "VALIDATION_FAILED", "bad input");
        assert_error(ApiError::planner("planner down"), "PLANNER_FAILED", "planner down");
        assert_error(ApiError::execution("command failed"), "EXECUTION_FAILED", "command failed");
        assert_error(ApiError::session_not_found("missing"), "SESSION_NOT_FOUND", "missing");
        assert_error(ApiError::database("locked"), "DATABASE_ERROR", "locked");

        let with_details = ApiError {
            code: ErrorCode::SessionDisconnected,
            message: "gone".into(),
            details: Some("pipe closed".into()),
        };
        let json = serde_json::to_value(&with_details).unwrap();
        assert_eq!(json["code"], "SESSION_DISCONNECTED");
        assert_eq!(json["details"], "pipe closed");
        assert_eq!(with_details.to_string(), "gone");

        let not_implemented = ApiError {
            code: ErrorCode::NotImplemented,
            message: "later".into(),
            details: None,
        };
        assert_eq!(
            serde_json::to_value(&not_implemented).unwrap()["code"],
            "NOT_IMPLEMENTED"
        );
        let unknown = ApiError {
            code: ErrorCode::UnknownError,
            message: "unknown".into(),
            details: None,
        };
        assert_eq!(
            serde_json::to_value(&unknown).unwrap()["code"],
            "UNKNOWN_ERROR"
        );
    }

    #[test]
    fn persistence_missing_id_is_not_found_and_other_failures_stay_database() {
        let missing = [
            "history update: no history item with id h1",
            "workflow delete: no workflow with id w1",
            "memory delete: no memory item with id m1",
            "suggestion not pending: sg1",
            "suggestion not found: Query returned no rows",
            "dismiss suggestion: no suggestion with id sg1",
        ];
        for message in missing {
            assert_error(ApiError::from_persistence(message), "NOT_FOUND", message);
        }

        assert_error(
            ApiError::from_persistence("Database not initialized"),
            "DATABASE_ERROR",
            "Database not initialized",
        );
        assert_error(
            ApiError::from_persistence("Failed to open database: locked"),
            "DATABASE_ERROR",
            "Failed to open database: locked",
        );
        assert_error(
            ApiError::from_persistence("accept suggestion: disk full"),
            "DATABASE_ERROR",
            "accept suggestion: disk full",
        );
    }

    #[test]
    fn exited_shell_is_its_own_code_and_other_execution_failures_are_not() {
        let exited = "The shell in this session has exited; open a new session";
        assert_error(ApiError::from_execution(exited), "SESSION_EXITED", exited);
        assert_error(
            ApiError::from_execution("Session not found: s1"),
            "EXECUTION_FAILED",
            "Session not found: s1",
        );
        assert_error(
            ApiError::from_execution("No command is currently running"),
            "EXECUTION_FAILED",
            "No command is currently running",
        );
        assert_error(
            ApiError::from_execution("The shell in this session has exited"),
            "EXECUTION_FAILED",
            "The shell in this session has exited",
        );
    }
}
