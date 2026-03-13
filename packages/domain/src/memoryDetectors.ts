import type { HistoryItem } from "./history";
import type { MemoryItem, MemorySuggestion, MemorySuggestionKind } from "./memory";

// --- Types ---

export type DetectorInput = {
  history: HistoryItem[];
  existingSuggestions: MemorySuggestion[];
  existingMemory: MemoryItem[];
  projectRoot?: string;
};

// --- Command normalizer ---

export function normalizeCommand(cmd: string): { family: string; full: string } {
  const full = cmd.trim().replace(/\s+/g, " ");
  const parts = full.split(" ");
  // family = executable + first subcommand (e.g. "git status", "pnpm test")
  const family = parts.length >= 2 ? `${parts[0]} ${parts[1]}` : parts[0] ?? "";
  return { family, full };
}

// --- Helpers ---

function isDismissedOrExists(
  kind: MemorySuggestionKind,
  proposedValue: string,
  input: DetectorInput,
): boolean {
  // Check dismissed suggestions
  const dismissed = input.existingSuggestions.some(
    (s) =>
      s.kind === kind &&
      s.proposedValue === proposedValue &&
      s.status === "dismissed",
  );
  if (dismissed) return true;

  // Check pending suggestions
  const pending = input.existingSuggestions.some(
    (s) =>
      s.kind === kind &&
      s.proposedValue === proposedValue &&
      s.status === "pending",
  );
  if (pending) return true;

  // Check accepted memory items
  const accepted = input.existingMemory.some(
    (m) =>
      (m.kind === kind || m.kind === "common_directory") &&
      m.value === proposedValue,
  );
  return accepted;
}

function scaledConfidence(
  count: number,
  threshold: number,
  min: number,
  max: number,
): number {
  const scale = Math.min((count - threshold) / (threshold * 2), 1);
  return Math.round((min + scale * (max - min)) * 100) / 100;
}

function successfulHistory(history: HistoryItem[]): HistoryItem[] {
  return history.filter(
    (h) => h.status === "success" && (h.executedCommand ?? h.generatedCommand),
  );
}

function commandOf(h: HistoryItem): string {
  return h.executedCommand ?? h.generatedCommand ?? "";
}

// --- Detector A: Preferred CWD ---

const CWD_MIN_EXECUTIONS = 5;
const CWD_MIN_SESSIONS = 2;

export function detectPreferredCwd(input: DetectorInput): MemorySuggestion[] {
  const successful = successfulHistory(input.history);
  const cwdMap = new Map<string, { count: number; sessions: Set<string>; ids: string[] }>();

  for (const h of successful) {
    if (!h.cwd) continue;
    const entry = cwdMap.get(h.cwd) ?? { count: 0, sessions: new Set(), ids: [] };
    entry.count++;
    entry.sessions.add(h.sessionId);
    entry.ids.push(h.id);
    cwdMap.set(h.cwd, entry);
  }

  const results: MemorySuggestion[] = [];

  for (const [cwd, data] of cwdMap) {
    if (data.count < CWD_MIN_EXECUTIONS) continue;
    if (data.sessions.size < CWD_MIN_SESSIONS) continue;
    if (isDismissedOrExists("preferred_cwd", cwd, input)) continue;

    results.push({
      id: `cwd-${cwd}`,
      scope: input.projectRoot ? "project" : "global",
      projectRoot: input.projectRoot,
      kind: "preferred_cwd",
      label: `You've worked in ${cwd} across ${data.count} executions in ${data.sessions.size} sessions`,
      proposedKey: "workspace",
      proposedValue: cwd,
      confidence: scaledConfidence(data.count, CWD_MIN_EXECUTIONS, 0.7, 0.95),
      derivedFromHistoryIds: data.ids,
      status: "pending",
      createdAt: new Date().toISOString(),
    });
  }

  return results;
}

// --- Detector B: Recurring Command Family ---

const CMD_MIN_EXECUTIONS = 4;

export function detectRecurringCommands(
  input: DetectorInput,
): MemorySuggestion[] {
  const successful = successfulHistory(input.history);
  const familyMap = new Map<
    string,
    { count: number; sessions: Set<string>; ids: string[] }
  >();

  for (const h of successful) {
    const cmd = commandOf(h);
    if (!cmd) continue;
    const { family } = normalizeCommand(cmd);
    if (!family) continue;

    const entry = familyMap.get(family) ?? {
      count: 0,
      sessions: new Set(),
      ids: [],
    };
    entry.count++;
    entry.sessions.add(h.sessionId);
    entry.ids.push(h.id);
    familyMap.set(family, entry);
  }

  const results: MemorySuggestion[] = [];

  for (const [family, data] of familyMap) {
    if (data.count < CMD_MIN_EXECUTIONS) continue;
    if (isDismissedOrExists("recurring_command", family, input)) continue;

    const sessionLabel =
      data.sessions.size > 1
        ? ` across ${data.sessions.size} sessions`
        : "";

    results.push({
      id: `cmd-${family}`,
      scope: input.projectRoot ? "project" : "global",
      projectRoot: input.projectRoot,
      kind: "recurring_command",
      label: `You frequently run '${family}' (${data.count} times${sessionLabel})`,
      proposedKey: family,
      proposedValue: family,
      confidence: scaledConfidence(data.count, CMD_MIN_EXECUTIONS, 0.6, 0.9),
      derivedFromHistoryIds: data.ids,
      status: "pending",
      createdAt: new Date().toISOString(),
    });
  }

  return results;
}

// --- Detector C: Command Sequence ---

const SEQ_MIN_OCCURRENCES = 3;
const SEQ_MIN_SESSIONS = 2;

export function detectWorkflowPatterns(
  input: DetectorInput,
): MemorySuggestion[] {
  const successful = successfulHistory(input.history);

  // Group by session, sort by createdAt
  const sessionGroups = new Map<string, HistoryItem[]>();
  for (const h of successful) {
    const group = sessionGroups.get(h.sessionId) ?? [];
    group.push(h);
    sessionGroups.set(h.sessionId, group);
  }

  // Count pairs and triples
  const pairCounts = new Map<
    string,
    { count: number; sessions: Set<string>; ids: Set<string> }
  >();
  const tripleCounts = new Map<
    string,
    { count: number; sessions: Set<string>; ids: Set<string> }
  >();

  for (const [sessionId, items] of sessionGroups) {
    const sorted = items.sort(
      (a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime(),
    );
    const families = sorted.map((h) => normalizeCommand(commandOf(h)).family);

    // Sliding window for pairs
    for (let i = 0; i < families.length - 1; i++) {
      const pair = `${families[i]} → ${families[i + 1]}`;
      const entry = pairCounts.get(pair) ?? {
        count: 0,
        sessions: new Set(),
        ids: new Set(),
      };
      entry.count++;
      entry.sessions.add(sessionId);
      entry.ids.add(sorted[i]!.id);
      entry.ids.add(sorted[i + 1]!.id);
      pairCounts.set(pair, entry);
    }

    // Sliding window for triples
    for (let i = 0; i < families.length - 2; i++) {
      const triple = `${families[i]} → ${families[i + 1]} → ${families[i + 2]}`;
      const entry = tripleCounts.get(triple) ?? {
        count: 0,
        sessions: new Set(),
        ids: new Set(),
      };
      entry.count++;
      entry.sessions.add(sessionId);
      entry.ids.add(sorted[i]!.id);
      entry.ids.add(sorted[i + 1]!.id);
      entry.ids.add(sorted[i + 2]!.id);
      tripleCounts.set(triple, entry);
    }
  }

  const results: MemorySuggestion[] = [];

  // Prefer triples over pairs when both exist
  const emittedPairs = new Set<string>();

  for (const [triple, data] of tripleCounts) {
    if (data.count < SEQ_MIN_OCCURRENCES) continue;
    if (data.sessions.size < SEQ_MIN_SESSIONS) continue;

    const parts = triple.split(" → ");
    const value = JSON.stringify(parts);
    if (isDismissedOrExists("workflow_pattern", value, input)) continue;

    results.push({
      id: `seq-${triple}`,
      scope: input.projectRoot ? "project" : "global",
      projectRoot: input.projectRoot,
      kind: "workflow_pattern",
      label: `You often run: ${triple}`,
      proposedKey: triple,
      proposedValue: value,
      confidence: scaledConfidence(data.count, SEQ_MIN_OCCURRENCES, 0.65, 0.85),
      derivedFromHistoryIds: [...data.ids],
      status: "pending",
      createdAt: new Date().toISOString(),
    });

    // Mark constituent pairs as covered
    for (let i = 0; i < parts.length - 1; i++) {
      emittedPairs.add(`${parts[i]} → ${parts[i + 1]}`);
    }
  }

  for (const [pair, data] of pairCounts) {
    if (data.count < SEQ_MIN_OCCURRENCES) continue;
    if (data.sessions.size < SEQ_MIN_SESSIONS) continue;
    if (emittedPairs.has(pair)) continue;

    const parts = pair.split(" → ");
    const value = JSON.stringify(parts);
    if (isDismissedOrExists("workflow_pattern", value, input)) continue;

    results.push({
      id: `seq-${pair}`,
      scope: input.projectRoot ? "project" : "global",
      projectRoot: input.projectRoot,
      kind: "workflow_pattern",
      label: `You often run: ${pair}`,
      proposedKey: pair,
      proposedValue: value,
      confidence: scaledConfidence(data.count, SEQ_MIN_OCCURRENCES, 0.65, 0.85),
      derivedFromHistoryIds: [...data.ids],
      status: "pending",
      createdAt: new Date().toISOString(),
    });
  }

  return results;
}

// --- Main entry ---

export function runDetectors(input: DetectorInput): MemorySuggestion[] {
  return [
    ...detectPreferredCwd(input),
    ...detectRecurringCommands(input),
    ...detectWorkflowPatterns(input),
  ];
}
