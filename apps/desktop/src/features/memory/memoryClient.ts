import { tauriInvoke } from "../../lib/tauriInvoke";
import type {
  MemoryListResponse,
  MemoryAddRequest,
  MemoryAddResponse,
  MemoryAcceptSuggestionRequest,
  MemoryAcceptSuggestionResponse,
  MemoryDismissSuggestionRequest,
  MemoryDismissSuggestionResponse,
  MemoryStoreSuggestionRequest,
  MemoryStoreSuggestionResponse,
  MemoryDeleteRequest,
  MemoryDeleteResponse,
} from "@commandui/api-contract";

export function memoryList(): Promise<MemoryListResponse> {
  return tauriInvoke("memory_list", {});
}

/** Ids and final status of every accepted or dismissed suggestion (not part of the shared contract yet). */
export function memoryListResolvedSuggestions(): Promise<{
  resolved: Array<{ id: string; status: string }>;
}> {
  return tauriInvoke("memory_list_resolved_suggestions", {});
}

export function memoryAdd(
  request: MemoryAddRequest,
): Promise<MemoryAddResponse> {
  return tauriInvoke("memory_add", { request });
}

export function memoryAcceptSuggestion(
  request: MemoryAcceptSuggestionRequest,
): Promise<MemoryAcceptSuggestionResponse> {
  return tauriInvoke("memory_accept_suggestion", { request });
}

export function memoryDismissSuggestion(
  request: MemoryDismissSuggestionRequest,
): Promise<MemoryDismissSuggestionResponse> {
  return tauriInvoke("memory_dismiss_suggestion", { request });
}

export function memoryStoreSuggestion(
  request: MemoryStoreSuggestionRequest,
): Promise<MemoryStoreSuggestionResponse> {
  return tauriInvoke("memory_store_suggestion", { request });
}

export function memoryDelete(
  request: MemoryDeleteRequest,
): Promise<MemoryDeleteResponse> {
  return tauriInvoke("memory_delete", { request });
}
