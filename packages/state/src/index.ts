import { create } from "zustand";
import type {
  HistoryItem,
  MemoryItem,
  MemorySuggestion,
  SessionSummary,
  Workflow,
} from "@commandui/domain";

export * from "./memorySelectors";

// --- Composer Store ---

type ComposerState = {
  inputValue: string;
  inputMode: "command" | "ask";
  setInputValue: (value: string) => void;
  setInputMode: (mode: "command" | "ask") => void;
};

export const useComposerStore = create<ComposerState>((set) => ({
  inputValue: "",
  inputMode: "command",
  setInputValue: (inputValue) => set({ inputValue }),
  setInputMode: (inputMode) => set({ inputMode }),
}));

// --- Execution Store ---

type ExecutionState = {
  activeExecutionId: string | null;
  lastExecutionId: string | null;
  executionStatus: "idle" | "running" | "success" | "failure";
  setActiveExecution: (executionId: string | null) => void;
  setExecutionStatus: (
    status: "idle" | "running" | "success" | "failure",
  ) => void;
  setLastExecutionId: (executionId: string | null) => void;
};

export const useExecutionStore = create<ExecutionState>((set) => ({
  activeExecutionId: null,
  lastExecutionId: null,
  executionStatus: "idle",
  setActiveExecution: (activeExecutionId) => set({ activeExecutionId }),
  setExecutionStatus: (executionStatus) => set({ executionStatus }),
  setLastExecutionId: (lastExecutionId) => set({ lastExecutionId }),
}));

// --- History Store ---

type HistoryState = {
  items: HistoryItem[];
  appendHistoryItem: (item: HistoryItem) => void;
  updateHistoryItem: (id: string, patch: Partial<HistoryItem>) => void;
  clearHistory: () => void;
};

export const useHistoryStore = create<HistoryState>((set) => ({
  items: [],
  appendHistoryItem: (item) =>
    set((state) => ({ items: [item, ...state.items] })),
  updateHistoryItem: (id, patch) =>
    set((state) => ({
      items: state.items.map((item) =>
        item.id === id ? { ...item, ...patch } : item,
      ),
    })),
  clearHistory: () => set({ items: [] }),
}));

// --- Session Store ---

type SessionState = {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  setSessions: (sessions: SessionSummary[]) => void;
  addSession: (session: SessionSummary) => void;
  removeSession: (sessionId: string) => void;
  setActiveSessionId: (sessionId: string | null) => void;
  updateSession: (
    sessionId: string,
    patch: Partial<SessionSummary>,
  ) => void;
};

export const useSessionStore = create<SessionState>((set) => ({
  sessions: [],
  activeSessionId: null,
  setSessions: (sessions) => set({ sessions }),
  addSession: (session) =>
    set((state) => ({
      sessions: [...state.sessions, session],
      activeSessionId: state.activeSessionId ?? session.id,
    })),
  removeSession: (sessionId) =>
    set((state) => {
      const nextSessions = state.sessions.filter((s) => s.id !== sessionId);
      const nextActive =
        state.activeSessionId === sessionId
          ? nextSessions[0]?.id ?? null
          : state.activeSessionId;
      return { sessions: nextSessions, activeSessionId: nextActive };
    }),
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),
  updateSession: (sessionId, patch) =>
    set((state) => ({
      sessions: state.sessions.map((session) =>
        session.id === sessionId ? { ...session, ...patch } : session,
      ),
    })),
}));

// --- Memory Store ---

type MemoryState = {
  items: MemoryItem[];
  suggestions: MemorySuggestion[];
  setMemoryItems: (items: MemoryItem[]) => void;
  setMemorySuggestions: (
    suggestions:
      | MemorySuggestion[]
      | ((prev: MemorySuggestion[]) => MemorySuggestion[]),
  ) => void;
  addMemoryItem: (item: MemoryItem) => void;
  removeMemoryItem: (memoryId: string) => void;
  removeSuggestion: (suggestionId: string) => void;
};

export const useMemoryStore = create<MemoryState>((set) => ({
  items: [],
  suggestions: [],
  setMemoryItems: (items) => set({ items }),
  setMemorySuggestions: (suggestions) =>
    set((state) => {
      const next =
        typeof suggestions === "function"
          ? suggestions(state.suggestions)
          : suggestions;

      const seen = new Set<string>();
      const deduped = next.filter((s) => {
        const key = `${s.scope}:${s.projectRoot ?? ""}:${s.kind}:${s.proposedKey}:${s.proposedValue}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });

      return { suggestions: deduped };
    }),
  addMemoryItem: (item) =>
    set((state) => ({ items: [item, ...state.items] })),
  removeMemoryItem: (memoryId) =>
    set((state) => ({
      items: state.items.filter((item) => item.id !== memoryId),
    })),
  removeSuggestion: (suggestionId) =>
    set((state) => ({
      suggestions: state.suggestions.filter((s) => s.id !== suggestionId),
    })),
}));

// --- Settings Store ---

type SettingsState = {
  productMode: "classic" | "guided";
  reducedClutter: boolean;
  simplifiedSummaries: boolean;
  confirmMediumRisk: boolean;
  defaultInputMode: "command" | "ask";
  setProductMode: (mode: "classic" | "guided") => void;
  setReducedClutter: (value: boolean) => void;
  setSimplifiedSummaries: (value: boolean) => void;
  setConfirmMediumRisk: (value: boolean) => void;
  setDefaultInputMode: (mode: "command" | "ask") => void;
};

export const useSettingsStore = create<SettingsState>((set) => ({
  productMode: "classic",
  reducedClutter: false,
  simplifiedSummaries: false,
  confirmMediumRisk: true,
  defaultInputMode: "command",
  setProductMode: (productMode) => set({ productMode }),
  setReducedClutter: (reducedClutter) => set({ reducedClutter }),
  setSimplifiedSummaries: (simplifiedSummaries) =>
    set({ simplifiedSummaries }),
  setConfirmMediumRisk: (confirmMediumRisk) => set({ confirmMediumRisk }),
  setDefaultInputMode: (defaultInputMode) => set({ defaultInputMode }),
}));

// --- Workflow Store ---

type WorkflowState = {
  items: Workflow[];
  setWorkflows: (workflows: Workflow[]) => void;
  addWorkflow: (workflow: Workflow) => void;
};

export const useWorkflowStore = create<WorkflowState>((set) => ({
  items: [],
  setWorkflows: (items) => set({ items }),
  addWorkflow: (workflow) =>
    set((state) => ({ items: [workflow, ...state.items] })),
}));
