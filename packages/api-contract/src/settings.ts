import type { SettingsSnapshot } from "@commandui/domain";

export type SettingsGetResponse = {
  settings: SettingsSnapshot;
};

export type SettingsUpdateRequest = {
  settings: Partial<SettingsSnapshot>;
};

export type SettingsUpdateResponse = {
  ok: boolean;
};
