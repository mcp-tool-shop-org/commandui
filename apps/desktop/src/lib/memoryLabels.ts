import { displayPath } from "./displayPath";

/** Plain names for memory kinds. The stored kind stays the code name. */
const KIND_LABELS: Record<string, string> = {
  preferred_cwd: "Preferred workspace",
  recurring_command: "Frequent command",
  workflow_pattern: "Workflow pattern",
  tool_preference: "Tool preference",
  preferred_mode: "Preferred mode",
  accepted_substitution: "Command substitution",
  common_directory: "Common directory",
  preferred_package_manager: "Package manager",
  preferred_search_tool: "Search tool",
  preferred_test_command: "Test command",
};

export function memoryKindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind;
}

/** Where a memory item applies, with the home folder shortened to ~. */
export function memoryScopeLabel(scope: string, projectRoot?: string | null): string {
  if (scope === "project") {
    return projectRoot ? `Only in ${displayPath(projectRoot)}` : "Only in its project folder";
  }
  return "Everywhere";
}

const FOLDER_KINDS = new Set(["preferred_cwd", "common_directory"]);

/** A memory value as shown: a folder is shortened like the header's. */
export function memoryValueLabel(kind: string, value: string): string {
  return FOLDER_KINDS.has(kind) ? displayPath(value) : value;
}

/**
 * A suggestion's sentence as shown. The detectors write the full folder into
 * the sentence; this swaps in the shortened one, so the account name stays off
 * the screen.
 */
export function suggestionLabel(kind: string, label: string, proposedValue: string): string {
  if (!FOLDER_KINDS.has(kind) || !proposedValue) return label;
  return label.split(proposedValue).join(displayPath(proposedValue));
}
