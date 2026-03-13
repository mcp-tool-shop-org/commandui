import { tauriInvoke } from "../../lib/tauriInvoke";
import type {
  SessionCreateRequest,
  SessionCreateResponse,
  SessionListResponse,
  SessionCloseRequest,
  SessionCloseResponse,
  TerminalExecuteRequest,
  TerminalExecuteResponse,
  TerminalResizeRequest,
  TerminalResizeResponse,
  TerminalWriteRequest,
  TerminalWriteResponse,
  TerminalInterruptRequest,
  TerminalInterruptResponse,
  TerminalResyncRequest,
  TerminalResyncResponse,
} from "@commandui/api-contract";

export function createSession(
  request: SessionCreateRequest,
): Promise<SessionCreateResponse> {
  return tauriInvoke("session_create", { request });
}

export function listSessions(): Promise<SessionListResponse> {
  return tauriInvoke("session_list", {});
}

export function closeSession(
  request: SessionCloseRequest,
): Promise<SessionCloseResponse> {
  return tauriInvoke("session_close", { request });
}

export function executeCommand(
  request: TerminalExecuteRequest,
): Promise<TerminalExecuteResponse> {
  return tauriInvoke("terminal_execute", { request });
}

export function resizeTerminal(
  request: TerminalResizeRequest,
): Promise<TerminalResizeResponse> {
  return tauriInvoke("terminal_resize", { request });
}

export function writeTerminal(
  request: TerminalWriteRequest,
): Promise<TerminalWriteResponse> {
  return tauriInvoke("terminal_write", { request });
}

export function interruptTerminal(
  request: TerminalInterruptRequest,
): Promise<TerminalInterruptResponse> {
  return tauriInvoke("terminal_interrupt", { request });
}

export function resyncTerminal(
  request: TerminalResyncRequest,
): Promise<TerminalResyncResponse> {
  return tauriInvoke("terminal_resync", { request });
}
