/**
 * Mock bridge for browser preview mode.
 * Returns plausible stub data so the UI is interactive without the Tauri backend.
 * Emits CustomEvents on window to simulate Tauri's event system.
 */

let sessionCounter = 0;

/** Emit a mock Tauri-style event that AppShell can subscribe to */
function emitMockEvent(eventName: string, payload: unknown) {
  window.dispatchEvent(
    new CustomEvent(`mock:${eventName}`, { detail: payload }),
  );
}

/** Subscribe to mock events (mirrors Tauri listen() API) */
export function onMockEvent<T>(
  eventName: string,
  callback: (payload: T) => void,
): () => void {
  const handler = (e: Event) => callback((e as CustomEvent).detail as T);
  window.addEventListener(`mock:${eventName}`, handler);
  return () => window.removeEventListener(`mock:${eventName}`, handler);
}

/** Fake command output for common commands */
function mockCommandOutput(command: string): string[] {
  const cmd = command.trim().toLowerCase();
  if (cmd.startsWith("echo ")) {
    return [command.slice(5)];
  }
  if (cmd === "ls" || cmd === "dir") {
    return [
      "README.md    package.json    src/",
      "node_modules/    tsconfig.json    .gitignore",
    ];
  }
  if (cmd === "pwd" || cmd === "cd") {
    return ["~/projects"];
  }
  if (cmd.startsWith("git status")) {
    return [
      "On branch main",
      "nothing to commit, working tree clean",
    ];
  }
  if (cmd.startsWith("git log")) {
    return [
      "abc1234 fix: browser preview mode (2 hours ago)",
      "def5678 feat: initial v0 scaffold (3 hours ago)",
    ];
  }
  return [`[mock] ${command}`, "(browser preview — command not executed)"];
}

function uuid(): string {
  return crypto.randomUUID?.() ?? `mock-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

const mockSessions: Record<string, Record<string, unknown>> = {};
const mockHistory: Array<Record<string, unknown>> = [];
const mockWorkflows: Array<Record<string, unknown>> = [];
const mockMemoryItems: Array<Record<string, unknown>> = [];
const mockRunningExecs = new Set<string>();

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  session_create(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    sessionCounter++;
    const now = new Date().toISOString();
    const session = {
      id: uuid(),
      label: (req.label as string) || `Session ${sessionCounter}`,
      cwd: (req.cwd as string) || "~/projects",
      shell: "mock-shell",
      status: "active" as const,
      createdAt: now,
      lastActiveAt: now,
    };
    mockSessions[session.id] = session;

    // Simulate boot → ready
    setTimeout(() => {
      emitMockEvent("session:exec_state_changed", {
        sessionId: session.id,
        execState: "ready",
        changedAt: new Date().toISOString(),
      });
      emitMockEvent("session:ready", {
        sessionId: session.id,
        cwd: session.cwd,
      });
    }, 100);

    return { session };
  },

  session_list() {
    return { sessions: Object.values(mockSessions) };
  },

  session_close(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    delete mockSessions[req.sessionId as string];
    return { ok: true };
  },

  session_update_cwd() {
    return { ok: true };
  },

  terminal_execute(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const executionId = (req.executionId as string) ?? uuid();
    const sessionId = req.sessionId as string;
    const command = (req.command as string) ?? "echo mock";

    // Track for interrupt support
    const execKey = `${sessionId}:${executionId}`;
    mockRunningExecs.add(execKey);

    // Simulate async PTY output
    setTimeout(() => {
      emitMockEvent("session:exec_state_changed", {
        sessionId,
        execState: "running",
        changedAt: new Date().toISOString(),
      });
      emitMockEvent("terminal:execution_started", {
        execution: { id: executionId, sessionId, command },
      });

      // Emit the command echo
      emitMockEvent("terminal:line", {
        sessionId,
        executionId,
        text: `$ ${command}\r\n`,
      });
    }, 50);

    // Emit output lines with staggered timing
    const outputLines = mockCommandOutput(command);
    outputLines.forEach((line, i) => {
      setTimeout(() => {
        if (!mockRunningExecs.has(execKey)) return;
        emitMockEvent("terminal:line", { sessionId, executionId, text: `${line}\r\n` });
      }, 150 + i * 80);
    });

    // Emit prompt + execution finished
    setTimeout(() => {
      const wasInterrupted = !mockRunningExecs.has(execKey);
      mockRunningExecs.delete(execKey);

      emitMockEvent("terminal:line", {
        sessionId,
        text: "~/projects $ ",
      });
      emitMockEvent("terminal:execution_finished", {
        executionId,
        sessionId,
        status: wasInterrupted ? "interrupted" : "success",
        exitCode: wasInterrupted ? 130 : 0,
      });
      emitMockEvent("session:exec_state_changed", {
        sessionId,
        execState: "ready",
        changedAt: new Date().toISOString(),
      });
    }, 200 + outputLines.length * 80);

    return { executionId, command };
  },

  terminal_interrupt(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const sessionId = req.sessionId as string;

    // Mark all running execs for this session as interrupted
    for (const key of mockRunningExecs) {
      if (key.startsWith(`${sessionId}:`)) {
        mockRunningExecs.delete(key);
      }
    }

    emitMockEvent("session:exec_state_changed", {
      sessionId,
      execState: "interrupting",
      changedAt: new Date().toISOString(),
    });

    return { ok: true };
  },

  terminal_resync(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const sessionId = req.sessionId as string;

    setTimeout(() => {
      emitMockEvent("session:exec_state_changed", {
        sessionId,
        execState: "ready",
        changedAt: new Date().toISOString(),
      });
    }, 200);

    return { ok: true };
  },

  terminal_resize() {
    return { ok: true };
  },

  terminal_write() {
    return { ok: true };
  },

  planner_generate_plan(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const intent = (req.userIntent as string) || "do something";
    const planId = uuid();
    return {
      plan: {
        id: planId,
        sessionId: req.sessionId ?? "mock",
        source: "semantic",
        userIntent: intent,
        command: `echo "mock plan for: ${intent}"`,
        explanation: `[Browser Preview] This would execute a plan for "${intent}". In Tauri mode, the AI planner generates real commands.`,
        assumptions: [],
        confidence: 0.85,
        risk: "low",
        destructive: false,
        requiresConfirmation: false,
        touchesFiles: false,
        touchesNetwork: false,
        escalatesPrivileges: false,
        generatedAt: new Date().toISOString(),
      },
      review: {
        planId,
        ambiguityFlags: [],
        safetyFlags: [],
        memoryUsed: [],
        retrievedContext: [],
      },
    };
  },

  history_append(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const item = req.item as Record<string, unknown> ?? { id: uuid(), ...req, createdAt: new Date().toISOString() };
    mockHistory.unshift(item);
    return { ok: true };
  },

  history_list(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const sessionId = req.sessionId as string | undefined;
    const items = sessionId
      ? mockHistory.filter((h) => (h as Record<string, unknown>).sessionId === sessionId)
      : mockHistory;
    return { items };
  },

  history_update(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const id = req.historyId as string;
    const idx = mockHistory.findIndex((h) => (h as Record<string, unknown>).id === id);
    if (idx >= 0) {
      const item = mockHistory[idx] as Record<string, unknown>;
      if (req.status !== undefined) item.status = req.status;
      if (req.exitCode !== undefined) item.exitCode = req.exitCode;
      if (req.executedCommand !== undefined) item.executedCommand = req.executedCommand;
      if (req.finishedAt !== undefined) item.finishedAt = req.finishedAt;
      if (req.durationMs !== undefined) item.durationMs = req.durationMs;
    }
    return { ok: true };
  },

  plan_store() {
    return { ok: true };
  },

  workflow_add(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const wf = { id: uuid(), ...req, createdAt: new Date().toISOString() };
    mockWorkflows.push(wf);
    return { workflow: wf };
  },

  workflow_list() {
    return { workflows: mockWorkflows };
  },

  settings_get() {
    return {
      settings: {
        productMode: "guided",
        defaultInputMode: "ask",
        reducedClutter: false,
        simplifiedSummaries: false,
        confirmMediumRisk: true,
      },
    };
  },

  settings_update() {
    return { ok: true };
  },

  memory_list() {
    return { items: mockMemoryItems, suggestions: [] };
  },

  memory_add(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    const item = { id: uuid(), ...req, createdAt: new Date().toISOString() };
    mockMemoryItems.push(item);
    return { item };
  },

  memory_accept_suggestion(args) {
    const req = (args.request ?? {}) as Record<string, unknown>;
    return {
      createdItem: {
        id: uuid(),
        scope: "project",
        kind: "accepted_substitution",
        key: "mock-key",
        value: "mock-value",
        confidence: 0.9,
        source: "suggestion",
        suggestionId: req.suggestionId,
        createdAt: new Date().toISOString(),
      },
    };
  },

  memory_dismiss_suggestion() {
    return { ok: true };
  },

  memory_delete() {
    return { ok: true };
  },
};

export function mockInvoke<T>(command: string, args?: Record<string, unknown>): T {
  const handler = handlers[command];
  if (handler) {
    return handler(args ?? {}) as T;
  }
  console.warn(`[mock-bridge] Unknown command: ${command}`);
  return { ok: true } as T;
}
