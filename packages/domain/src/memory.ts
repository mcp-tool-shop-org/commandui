export type MemoryItem = {
  id: string;
  scope: "global" | "project";
  projectRoot?: string;
  kind:
    | "preferred_package_manager"
    | "preferred_search_tool"
    | "preferred_test_command"
    | "accepted_substitution"
    | "common_directory"
    | "preferred_cwd"
    | "recurring_command"
    | "workflow_pattern"
    | "tool_preference"
    | "preferred_mode";
  key: string;
  value: string;
  confidence: number;
  source: "observed" | "accepted" | "manual";
  createdAt: string;
  updatedAt: string;
};

export type MemorySuggestionKind = MemoryItem["kind"];

export type MemorySuggestion = {
  id: string;
  scope: "global" | "project";
  projectRoot?: string;
  kind: MemoryItem["kind"];
  label: string;
  proposedKey: string;
  proposedValue: string;
  confidence: number;
  derivedFromHistoryIds: string[];
  status: "pending" | "accepted" | "dismissed";
  createdAt: string;
};
