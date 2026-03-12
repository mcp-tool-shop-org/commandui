import { tauriInvoke } from "../../lib/tauriInvoke";
import type {
  MemoryListResponse,
  MemoryAddRequest,
  MemoryAddResponse,
  MemoryAcceptSuggestionRequest,
  MemoryAcceptSuggestionResponse,
  MemoryDismissSuggestionRequest,
  MemoryDismissSuggestionResponse,
  MemoryDeleteRequest,
  MemoryDeleteResponse,
} from "@commandui/api-contract";

export function memoryList(): Promise<MemoryListResponse> {
  return tauriInvoke("memory_list", {});
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

export function memoryDelete(
  request: MemoryDeleteRequest,
): Promise<MemoryDeleteResponse> {
  return tauriInvoke("memory_delete", { request });
}
