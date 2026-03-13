/**
 * Mock bridge for browser preview mode.
 * Returns plausible stub data so the UI is interactive without the Tauri backend.
 */

let sessionCounter = 0;

function uuid(): string {
  return crypto.randomUUID?.() ?? `mock-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

const mockSessions: Record<string, Record<string, unknown>> = {};
const mockHistory: Array<Record<string, unknown>> = [];
const mockWorkflows: Array<Record<string, unknown>> = [];
const mockMemoryItems: Array<Record<string, unknown>> = [];

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
    return { executionId: req.executionId ?? uuid(), command: req.command ?? "echo mock" };
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
    const item = { id: uuid(), ...req, createdAt: new Date().toISOString() };
    mockHistory.unshift(item);
    return { item };
  },

  history_list() {
    return { items: mockHistory };
  },

  history_update() {
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
