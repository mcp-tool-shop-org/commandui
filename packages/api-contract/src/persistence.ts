import type {
  HistoryItem,
  MemoryItem,
  MemorySuggestion,
  SessionSummary,
  SettingsSnapshot,
  Workflow,
} from "@commandui/domain";

export type PersistenceHydrateRequest = {
  includeRecentTerminalOutput?: boolean;
  recentTerminalOutputLimit?: number;
};

export type PersistenceHydrateResponse = {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  historyItems: HistoryItem[];
  memoryItems: MemoryItem[];
  memorySuggestions: MemorySuggestion[];
  workflows: Workflow[];
  settings: SettingsSnapshot;
};
