import { tauriInvoke } from "../../lib/tauriInvoke";
import type {
  HistoryAppendRequest,
  HistoryAppendResponse,
  HistoryListRequest,
  HistoryListResponse,
  HistoryUpdateRequest,
  HistoryUpdateResponse,
  PlanStoreRequest,
  PlanStoreResponse,
  WorkflowAddRequest,
  WorkflowAddResponse,
  WorkflowDeleteRequest,
  WorkflowDeleteResponse,
  WorkflowListResponse,
  SettingsGetResponse,
  SettingsUpdateRequest,
  SettingsUpdateResponse,
} from "@commandui/api-contract";

export function historyAppend(
  request: HistoryAppendRequest,
): Promise<HistoryAppendResponse> {
  return tauriInvoke("history_append", { request });
}

export function historyList(
  request: HistoryListRequest,
): Promise<HistoryListResponse> {
  return tauriInvoke("history_list", { request });
}

export function historyUpdate(
  request: HistoryUpdateRequest,
): Promise<HistoryUpdateResponse> {
  return tauriInvoke("history_update", { request });
}

export function planStore(
  request: PlanStoreRequest,
): Promise<PlanStoreResponse> {
  return tauriInvoke("plan_store", { request });
}

export function workflowAdd(
  request: WorkflowAddRequest,
): Promise<WorkflowAddResponse> {
  return tauriInvoke("workflow_add", { request });
}

export function workflowList(): Promise<WorkflowListResponse> {
  return tauriInvoke("workflow_list", {});
}

export function workflowDelete(
  request: WorkflowDeleteRequest,
): Promise<WorkflowDeleteResponse> {
  return tauriInvoke("workflow_delete", { request });
}

export function settingsGet(): Promise<SettingsGetResponse> {
  return tauriInvoke("settings_get", {});
}

export function settingsUpdate(
  request: SettingsUpdateRequest,
): Promise<SettingsUpdateResponse> {
  return tauriInvoke("settings_update", { request });
}
