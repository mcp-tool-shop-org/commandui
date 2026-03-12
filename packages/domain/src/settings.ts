export type SettingsSnapshot = {
  productMode: "classic" | "guided";
  theme: "system" | "light" | "dark";
  fontSize: "sm" | "md" | "lg";
  density: "compact" | "comfortable";
  defaultInputMode: "command" | "ask";
  autoOpenPlanPanel: boolean;
  confirmMediumRisk: boolean;
  explanationVerbosity: "brief" | "normal";
  reducedClutter: boolean;
  simplifiedSummaries: boolean;
};
