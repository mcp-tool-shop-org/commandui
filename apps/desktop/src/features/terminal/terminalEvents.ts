import { listen } from "@tauri-apps/api/event";
import type {
  TerminalLineEvent,
  TerminalExecutionStartedEvent,
  TerminalExecutionFinishedEvent,
  SessionCwdChangedEvent,
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
