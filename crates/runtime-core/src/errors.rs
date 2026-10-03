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
}
