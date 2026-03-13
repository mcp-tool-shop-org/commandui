import { listen } from "@tauri-apps/api/event";
import type {
  TerminalLineEvent,
  TerminalExecutionStartedEvent,
  TerminalExecutionFinishedEvent,
  SessionCwdChangedEvent,
  SessionReadyEvent,
  SessionExecStateChangedEvent,
} from "@commandui/api-contract";

export function subscribeToTerminalLines(
  onLine: (event: TerminalLineEvent) => void,
): Promise<() => void> {
  return listen<TerminalLineEvent>("terminal:line", (e) => onLine(e.payload));
}

export function subscribeToExecutionStarted(
  onStarted: (event: TerminalExecutionStartedEvent) => void,
): Promise<() => void> {
  return listen<TerminalExecutionStartedEvent>(
    "terminal:execution_started",
    (e) => onStarted(e.payload),
  );
}

export function subscribeToExecutionFinished(
  onFinished: (event: TerminalExecutionFinishedEvent) => void,
): Promise<() => void> {
  return listen<TerminalExecutionFinishedEvent>(
    "terminal:execution_finished",
    (e) => onFinished(e.payload),
  );
}

export function subscribeToSessionCwdChanged(
  onChanged: (event: SessionCwdChangedEvent) => void,
): Promise<() => void> {
  return listen<SessionCwdChangedEvent>("session:cwd_changed", (e) =>
    onChanged(e.payload),
  );
}

export function subscribeToSessionReady(
  onReady: (event: SessionReadyEvent) => void,
): Promise<() => void> {
  return listen<SessionReadyEvent>("session:ready", (e) =>
    onReady(e.payload),
  );
}

export function subscribeToExecStateChanged(
  onChanged: (event: SessionExecStateChangedEvent) => void,
): Promise<() => void> {
  return listen<SessionExecStateChangedEvent>(
    "session:exec_state_changed",
    (e) => onChanged(e.payload),
  );
}
