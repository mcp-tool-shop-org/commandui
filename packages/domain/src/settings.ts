export type SettingsSnapshot = {
  productMode: "classic" | "guided";
  /** "sm" and "md" are the old steps and mean 100%. "lg" means 200%. A percent from "100" to "200" is the slider. */
  fontSize: string;
  defaultInputMode: "command" | "ask";
  plannerModel: string;
  plannerEndpoint: string;
  simplifiedSummaries: boolean;
};
