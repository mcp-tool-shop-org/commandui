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
