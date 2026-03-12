import type { HistoryItem } from "@commandui/domain";

export type HistoryAppendRequest = {
  item: HistoryItem;
};

export type HistoryAppendResponse = {
  ok: boolean;
};

export type HistoryListRequest = {
  sessionId?: string;
  limit?: number;
};

export type HistoryListResponse = {
  items: HistoryItem[];
};

export type HistoryUpdateRequest = {
  historyId: string;
  status?: string;
  exitCode?: number;
  executedCommand?: string;
};

export type HistoryUpdateResponse = {
  ok: boolean;
};
