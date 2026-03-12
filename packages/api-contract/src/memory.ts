import type { MemoryItem, MemorySuggestion } from "@commandui/domain";

export type MemoryListResponse = {
  items: MemoryItem[];
  suggestions: MemorySuggestion[];
};

export type MemoryAddRequest = {
  item: MemoryItem;
};

export type MemoryAddResponse = {
  ok: boolean;
};

export type MemoryAcceptSuggestionRequest = {
  suggestionId: string;
};

export type MemoryAcceptSuggestionResponse = {
  ok: boolean;
  createdItem?: MemoryItem;
};

export type MemoryDismissSuggestionRequest = {
  suggestionId: string;
};

export type MemoryDismissSuggestionResponse = {
  ok: boolean;
};

export type MemoryDeleteRequest = {
  memoryId: string;
};

export type MemoryDeleteResponse = {
  ok: boolean;
};
