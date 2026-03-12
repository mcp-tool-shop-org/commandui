export type BackendError = {
  code:
    | "SESSION_NOT_FOUND"
    | "SESSION_DISCONNECTED"
    | "EXECUTION_FAILED"
    | "PLANNER_FAILED"
    | "VALIDATION_FAILED"
    | "DATABASE_ERROR"
    | "NOT_IMPLEMENTED"
    | "UNKNOWN_ERROR";
  message: string;
  details?: Record<string, unknown>;
};
