export type SettingsGetResponse = {
  settings: Record<string, unknown>;
};

export type SettingsUpdateRequest = {
  settings: Record<string, unknown>;
};

export type SettingsUpdateResponse = {
  ok: boolean;
};
