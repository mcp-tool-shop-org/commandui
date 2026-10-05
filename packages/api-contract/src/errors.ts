export type BackendError = {
  code:
    | "SESSION_NOT_FOUND"
    | "SESSION_DISCONNECTED"
    | "SESSION_EXITED"
    | "EXECUTION_FAILED"
    | "PLANNER_FAILED"
    | "VALIDATION_FAILED"
    | "DATABASE_ERROR"
    | "NOT_FOUND"
    | "NOT_IMPLEMENTED"
    | "UNKNOWN_ERROR";
  message: string;
  /** Extra sentence from Rust. A missing value arrives as null. */
  details?: string | null;
};
