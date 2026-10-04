import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";
import type {
  CommandPlan,
  HistoryItem,
  MemorySuggestion,
  SessionSummary,
  Workflow,
  WorkflowRun,
  WorkflowStepRun,
} from "@commandui/domain";
import { runDetectors } from "@commandui/domain";
import type {
  PlannerGeneratePlanResponse,
  PlannerStatus,
  SessionExecState,
  TerminalExecutionFinishedEvent,
  TerminalExecutionStartedEvent,
} from "@commandui/api-contract";
import {
  useComposerStore,
  useExecutionStore,
  useHistoryStore,
  useSessionStore,
  useMemoryStore,
  useSettingsStore,
  useWorkflowStore,
  useWorkflowRunStore,
  useFocusStore,
} from "@commandui/state";
import type { ShortcutDef } from "../lib/shortcuts";
import type { ShortcutContext } from "../lib/shortcuts";
import { buildPlannerContext } from "../lib/buildPlannerContext";
import { persistInBackground } from "../lib/persistInBackground";
import { useShortcuts } from "../hooks/useShortcuts";
import {
  createSession,
  listSessions,
  closeSession,
  executeCommand,
  resizeTerminal,
  writeTerminal,
  interruptTerminal,
  resyncTerminal,
} from "../features/terminal/terminalClient";
import {
  subscribeToTerminalLines,
  subscribeToExecutionStarted,
  subscribeToExecutionFinished,
  subscribeToSessionCwdChanged,
  subscribeToSessionReady,
  subscribeToExecStateChanged,
} from "../features/terminal/terminalEvents";
import { generatePlan } from "../features/planner/plannerClient";
import {
  historyAppend,
  historyList,
  historyUpdate,
  planStore,
  workflowAdd,
  workflowDelete,
  workflowList,
  settingsGet,
  settingsUpdate,
} from "../features/persistence/persistenceClient";
import {
  memoryList,
  memoryAcceptSuggestion,
  memoryDismissSuggestion,
  memoryDelete,
  memoryStoreSuggestion,
  memoryListResolvedSuggestions,
} from "../features/memory/memoryClient";
import { InputComposer } from "../components/InputComposer";
import type { InputComposerHandle } from "../components/InputComposer";
import { PlanPanel, planCanRun } from "../components/PlanPanel";
import { PlannerStatusCard } from "../components/PlannerStatusCard";
import { commandProblem, escapeForTerminal } from "../lib/displaySafe";
import {
  askFixPrompt,
  collapseRedraws,
  countOutputLines,
  describeResult,
  looksLikeRequest,
  resultText,
} from "../lib/commandResult";
import type { CommandResult, ResultAction } from "../lib/commandResult";
import type { PlanRunGate } from "../components/PlanPanel";
import { TerminalPane } from "../components/TerminalPane";
import { ResultLine } from "../components/ResultLine";
import type { TerminalPaneHandle } from "../components/TerminalPane";
import { CommandPalette } from "../components/CommandPalette";
import type { PaletteAction } from "../components/CommandPalette";
import { HistoryDrawer } from "../components/HistoryDrawer";
import { SessionTabs } from "../components/SessionTabs";
import { SettingsDrawer } from "../components/SettingsDrawer";
import { MemorySuggestions } from "../components/MemorySuggestions";
import { MemoryDrawer } from "../components/MemoryDrawer";
import { WorkflowDrawer } from "../components/WorkflowDrawer";
import { WorkflowEditor } from "../components/WorkflowEditor";
import { HelpDialog } from "../components/HelpDialog";
import { OutputView } from "../components/OutputView";
import type { OutputBlock } from "../components/OutputView";
import { WorkflowRunBanner } from "../components/WorkflowRunBanner";
import { isTauriRuntime } from "../lib/tauriInvoke";
import { errorText, isNotFoundError, isSessionExitedError } from "../lib/commandError";
import { recordedPlannerSource } from "../lib/plannerSource";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { onMockEvent } from "../lib/mockBridge";
import { displayPath } from "../lib/displayPath";
import { fontScale } from "../lib/fontScale";
import { readShowWelcome, writeShowWelcome } from "../lib/welcomePref";
import { WelcomeScreen } from "../components/WelcomeScreen";
import { waitForTerminalStatus } from "./workflowStepWait";
import {
  busyMessageFor,
  composerDisabled,
  execIsBusy,
  execIsForeground,
  execOwnsBadge,
} from "./execState";

const APP_VERSION = "1.0.2";
/** How long a delete can be undone, in milliseconds. */
const UNDO_MS = 10_000;

type SessionResult = {
  command: string;
  view: CommandResult;
  output: string;
  outputOpen: boolean;
  exitCode: number | null;
  exitKnown: boolean;
};

type ConfirmRequest = {
  title: string;
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
};

type UndoRequest = {
  token: string;
  message: string;
  restore: () => void;
  commit: () => void;
};
/** How long a session may stay in "booting" before the UI offers Resync and Close. */
const BOOT_STALL_MS = 20_000;
/** How long a typed command (or an interrupt) may hold a session before the UI explains the way out. */
const FOREGROUND_STUCK_MS = 30_000;
/** Cadence of the one timer that watches every session's boot and foreground time. */
const SESSION_WATCH_MS = 2_000;
const SESSION_NOT_READY_MESSAGE =
  "The terminal is not ready yet. Wait for the session to finish starting, or resync it.";
const SESSION_EXITED_MESSAGE =
  "The shell in this session has exited. Open a new session to continue.";
/**
 * Per-session replay buffer cap, in characters (not entries: a PTY read can be 4 KB, so
 * an entry cap let one session hold tens of MB). xterm keeps its own scrollback.
 */
const TERMINAL_REPLAY_MAX_CHARS = 600_000;
/** Chunks are merged up to this size so a flood of tiny reads stays a few entries. */
const TERMINAL_REPLAY_CHUNK_CHARS = 16_000;
/** SGR reset, then a marker: truncation can cut mid-escape and mid-line, so start clean. */
const TERMINAL_REPLAY_TRUNCATED_MARKER = "\x1b[0m[earlier output truncated]\r\n";
/**
 * Truncation of a session that is inside a full-screen app (alternate screen) must not
 * start the replay on the main screen, or the app's cursor-addressed frames land on the
 * shell's scrollback. Replay re-enters the alternate screen first.
 */
const TERMINAL_REPLAY_ALT_PREFIX = "\x1b[0m\x1b[?1049h";
const ALT_SCREEN_SEQ = /\x1b\[\?(?:1049|1047|47)([hl])/g;
const ALT_SCREEN_TAIL_CHARS = 12;
/**
 * Private modes the shell sets (DECSET/DECRST) that a truncated or cleared replay would lose,
 * and xterm's reset would turn off: cursor keys, mouse and focus reporting, bracketed paste.
 * The alternate screen has its own tracking above.
 */
const TRACKED_PRIVATE_MODES: ReadonlySet<number> = new Set([
  1, 9, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1015, 1016, 2004,
]);
const PRIVATE_MODE_SEQ = /\x1b\[\?([\d;]+)([hl])/g;
const PRIVATE_MODE_TAIL_CHARS = 32;
const EXEC_STATES: readonly SessionExecState[] = [
  "booting",
  "ready",
  "running",
  "interrupting",
  "desynced",
  "userRunning",
];
const APP_NOTES_MAX = 200;
const APP_NOTES_SHOWN = 6;

function execStateOf(sessionId: string): SessionExecState | undefined {
  return useExecutionStore.getState().sessionExecStates[sessionId];
}

function busyMessage(sessionId: string): string {
  return busyMessageFor(execStateOf(sessionId));
}

/** Fold the DECSET/DECRST sequences in `text` into the last-known value per tracked mode. */
function privateModesAfter(previous: Record<number, boolean>, text: string): Record<number, boolean> {
  let next = previous;
  for (const match of text.matchAll(PRIVATE_MODE_SEQ)) {
    for (const part of match[1].split(";")) {
      const mode = Number(part);
      if (!TRACKED_PRIVATE_MODES.has(mode)) continue;
      if (next === previous) next = { ...previous };
      next[mode] = match[2] === "h";
    }
  }
  return next;
}

/** The sequences that put a freshly reset xterm back into the modes the shell last set. */
function privateModePrefix(modes: Record<number, boolean> | undefined): string {
  if (!modes) return "";
  return Object.entries(modes)
    .map(([mode, on]) => `\x1b[?${mode}${on ? "h" : "l"}`)
    .join("");
}

function isReplayPrefix(entry: string): boolean {
  return entry === TERMINAL_REPLAY_TRUNCATED_MARKER || entry === TERMINAL_REPLAY_ALT_PREFIX;
}

function altScreenAfter(previous: boolean, text: string): boolean {
  let state = previous;
  for (const match of text.matchAll(ALT_SCREEN_SEQ)) state = match[1] === "h";
  return state;
}

type SessionSummaryWithState = SessionSummary & { execState?: string | null };

type SessionBadgeStatus = "idle" | "running" | "success" | "failure";

function simplifyText(text: string): string {
  const first = text.split(/[.!?]\s/)[0];
  return first + (first.endsWith(".") ? "" : ".");
}

function detectOS(): "windows" | "macos" | "linux" {
  const p = navigator.platform.toLowerCase();
  if (p.includes("win")) return "windows";
  if (p.includes("mac")) return "macos";
  return "linux";
}

export function AppShell() {
  // --- Stores ---
  const { inputMode, setInputMode, setInputValue } = useComposerStore();
  const {
    setActiveExecution,
    setExecutionStatus,
    setLastExecutionId,
    sessionExecStates,
    setSessionExecState,
  } = useExecutionStore();
  const { items: historyItems, loadHistory, appendHistoryItem, updateHistoryItem } =
    useHistoryStore();
  const {
    sessions,
    activeSessionId,
    addSession,
    removeSession,
    setActiveSessionId,
    updateSession,
    setSessions,
  } = useSessionStore();
  const {
    items: memoryItems,
    suggestions: memorySuggestions,
    setMemoryItems,
    setMemorySuggestions,
    addMemoryItem,
    removeMemoryItem,
    removeSuggestion,
  } = useMemoryStore();
  const {
    productMode,
    fontSize,
    simplifiedSummaries,
    plannerModel,
    plannerEndpoint,
    defaultInputMode,
    setProductMode,
    setFontSize,
    setSimplifiedSummaries,
    setPlannerModel,
    setPlannerEndpoint,
    setDefaultInputMode,
  } = useSettingsStore();
  const { items: workflows, setWorkflows, addWorkflow, removeWorkflow } = useWorkflowStore();
  const { setActiveRun, updateActiveRunStep, completeActiveRun } = useWorkflowRunStore();
  const lastRunByWorkflowId = useWorkflowRunStore((s) => s.lastRunByWorkflowId);
  const { restorePreviousZone } = useFocusStore();

  // --- Local state ---
  const [plan, setPlan] = useState<{
    plan: CommandPlan;
    review: NonNullable<PlannerGeneratePlanResponse["review"]>;
    status: PlannerStatus;
  } | null>(null);
  const [plannerStatus, setPlannerStatus] = useState<PlannerStatus | null>(null);
  const [planNonce, setPlanNonce] = useState(0);
  const [currentPlanHistoryId, setCurrentPlanHistoryId] = useState<
    string | null
  >(null);
  const [busySessions, setBusySessions] = useState<ReadonlySet<string>>(new Set());
  const [sessionBadge, setSessionBadge] = useState<Record<string, SessionBadgeStatus>>({});
  const [error, setError] = useState<string | null>(null);
  const [bootPhase, setBootPhase] = useState<"booting" | "ready" | "failed">("booting");
  const [bootError, setBootError] = useState<string | null>(null);
  const [exitedSessions, setExitedSessions] = useState<ReadonlySet<string>>(new Set());
  const [planNotice, setPlanNotice] = useState<string | null>(null);
  // Lines the app itself writes ([approved], [plan], Welcome...) live here, outside xterm: the
  // shell's absolute cursor moves know nothing about them and would overwrite them.
  const [appNotes, setAppNotes] = useState<Record<string, string[]>>({});
  const [stalledBoot, setStalledBoot] = useState<ReadonlySet<string>>(new Set());
  const [stuckForeground, setStuckForeground] = useState<ReadonlySet<string>>(new Set());

  const [browserPreview] = useState(() => !isTauriRuntime());
  const [historyOpen, setHistoryOpen] = useState(false);
  const [workflowOpen, setWorkflowOpen] = useState(false);
  const [expandedRunWorkflowId, setExpandedRunWorkflowId] = useState<string | null>(null);
  const [historyInitialExpandedId, setHistoryInitialExpandedId] = useState<string | null>(null);
  const [memoryOpen, setMemoryOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // The welcome opens at launch until the user turns it off.
  const [showWelcomeAtStartup, setShowWelcomeAtStartup] = useState(() => readShowWelcome());
  const [welcomeOpen, setWelcomeOpen] = useState(showWelcomeAtStartup);
  const [pendingConfirm, setPendingConfirm] = useState<ConfirmRequest | null>(null);
  const confirmResolveRef = useRef<((accepted: boolean) => void) | null>(null);
  const pendingUndoRef = useRef<UndoRequest | null>(null);
  const undoTimerRef = useRef<number | null>(null);
  const [undoMessage, setUndoMessage] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [outputOpen, setOutputOpen] = useState(false);
  const [outputBlocksBySession, setOutputBlocksBySession] = useState<Record<string, OutputBlock[]>>({});
  const [liveMessage, setLiveMessage] = useState("");
  const [resultsBySession, setResultsBySession] = useState<Record<string, SessionResult>>({});
  const [requestOffers, setRequestOffers] = useState<Record<string, string>>({});
  const [workflowEditorData, setWorkflowEditorData] = useState<{
    workflowId: string;
    suggestionId?: string;
    label: string;
    steps: string[];
    projectRoot?: string;
    createdAt?: string;
    source?: Workflow["source"];
    originalIntent?: string;
    mode: "create" | "edit";
  } | null>(null);

  // Background buffer for session-switch replay
  // A ref (not state): appending must not re-render per PTY chunk. Capped per
  // session; xterm keeps the real scrollback.
  const terminalLinesBySessionRef = useRef<Record<string, string[]>>({});
  const executionToHistoryRef = useRef<Record<string, string>>({});
  const outputByExecRef = useRef<Record<string, string>>({});
  const commandByExecRef = useRef<Record<string, string>>({});
  const runningExecBySessionRef = useRef<Record<string, string>>({});
  const bootedRef = useRef(false);
  const settingsHydratedRef = useRef(false);
  const sessionBadgeRef = useRef<Record<string, SessionBadgeStatus>>({});
  const inFlightExecRef = useRef(new Set<string>());
  const busySessionsRef = useRef(new Set<string>());
  const workflowAbortBySessionRef = useRef(new Map<string, AbortController>());
  const terminalPaneRef = useRef<TerminalPaneHandle>(null);
  const composerRef = useRef<InputComposerHandle>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const planGateRef = useRef<PlanRunGate>({ command: "", confirmed: false });
  const approvePlanRef = useRef<(command: string) => void>(() => {});
  const planNonceSeenRef = useRef(0);
  const approveInFlightRef = useRef(false);
  const terminalIoFailedRef = useRef(new Set<string>());
  const exitedSessionsRef = useRef(new Set<string>());
  const replayCharsRef = useRef<Record<string, number>>({});
  const altScreenRef = useRef<Record<string, boolean>>({});
  const altTailRef = useRef<Record<string, string>>({});
  const privateModesRef = useRef<Record<string, Record<number, boolean>>>({});
  const privateModeTailRef = useRef<Record<string, string>>({});
  /** When each session entered its current exec state; stall timers read this, not a per-tab timer. */
  const execSinceRef = useRef<Record<string, { state: SessionExecState; at: number }>>({});
  const stalledBootRef = useRef<ReadonlySet<string>>(new Set());
  const stuckForegroundRef = useRef<ReadonlySet<string>>(new Set());
  /** Per-session tail of the keystroke chain, so writes reach the shell in the order typed. */
  const terminalWriteChainRef = useRef<Record<string, Promise<unknown>>>({});
  /** Suggestion ids the database already holds as accepted or dismissed (when the API reports them). */
  const settledSuggestionIdsRef = useRef(new Set<string>());

  const acceptPlanRunGate = useCallback((gate: PlanRunGate) => {
    planGateRef.current = gate;
  }, []);

  // Keep ref in sync with state (for use in event callbacks)
  activeSessionIdRef.current = activeSessionId;
  stalledBootRef.current = stalledBoot;
  stuckForegroundRef.current = stuckForeground;

  // A new plan (including a reopen of the same command) starts unconfirmed.
  if (planNonceSeenRef.current !== planNonce || plan === null) {
    planNonceSeenRef.current = planNonce;
    planGateRef.current = { command: "", confirmed: false };
  }

  const session =
    sessions.find((s) => s.id === activeSessionId) ?? null;

  const activeExecState: SessionExecState =
    (activeSessionId ? sessionExecStates[activeSessionId] : undefined) ?? "booting";
  const activeBootStalled =
    activeExecState === "booting" && activeSessionId !== null && stalledBoot.has(activeSessionId);
  const activeForegroundStuck =
    execIsForeground(activeExecState) && activeSessionId !== null && stuckForeground.has(activeSessionId);
  const activeNotes = activeSessionId ? appNotes[activeSessionId] ?? [] : [];
  const activeExited = activeSessionId ? exitedSessions.has(activeSessionId) : false;
  const activeBadge: SessionBadgeStatus =
    (activeSessionId && sessionBadge[activeSessionId]) || "idle";
  const isRunning =
    activeBadge === "running" ||
    activeExecState === "running" ||
    activeExecState === "interrupting" ||
    activeExecState === "userRunning";
  const visibleExecutionStatus: "idle" | "running" | "success" | "failure" | "interrupted" =
    isRunning ? "running" : activeBadge;
  const composerBusy = activeSessionId ? busySessions.has(activeSessionId) : false;

  function publishBadge(sessionId: string, status: SessionBadgeStatus) {
    const next = { ...sessionBadgeRef.current, [sessionId]: status };
    sessionBadgeRef.current = next;
    setSessionBadge(next);
    if (sessionId === activeSessionIdRef.current) {
      setExecutionStatus(status);
    }
  }

  function sessionIsRunning(sessionId: string | null | undefined): boolean {
    if (!sessionId) return false;
    if (inFlightExecRef.current.has(sessionId)) return true;
    if (sessionBadgeRef.current[sessionId] === "running") return true;
    return execIsBusy(execStateOf(sessionId));
  }

  function lockSession(sessionId: string) {
    const next = new Set(busySessionsRef.current);
    next.add(sessionId);
    busySessionsRef.current = next;
    setBusySessions(next);
  }

  function unlockSession(sessionId: string) {
    if (!busySessionsRef.current.has(sessionId)) return;
    const next = new Set(busySessionsRef.current);
    next.delete(sessionId);
    busySessionsRef.current = next;
    setBusySessions(next);
  }

  function clearSessionRunning(sessionId: string) {
    publishBadge(sessionId, "idle");
    const exec = useExecutionStore.getState().sessionExecStates[sessionId];
    if (exec === "running" || exec === "interrupting") {
      setSessionExecState(sessionId, "ready");
    }
  }

  const visibleHistoryItems = activeSessionId
    ? historyItems.filter((h) => h.sessionId === activeSessionId)
    : historyItems;

  // --- Helpers ---
  function clearTerminalView() {
    const sid = activeSessionIdRef.current;
    if (sid) {
      delete terminalLinesBySessionRef.current[sid];
      // The total must go with the buffer, or truncation starts early on stale counts.
      delete replayCharsRef.current[sid];
      delete altScreenRef.current[sid];
      delete altTailRef.current[sid];
      setAppNotes((prev) => {
        if (!(sid in prev)) return prev;
        const { [sid]: _removed, ...rest } = prev;
        return rest;
      });
    }
    terminalPaneRef.current?.clear();
  }

  function sessionNotReady(sessionId: string): boolean {
    if (exitedSessionsRef.current.has(sessionId)) return true;
    const exec = execStateOf(sessionId) ?? "booting";
    return exec === "booting" || exec === "desynced";
  }

  function notReadyMessage(sessionId: string): string {
    return exitedSessionsRef.current.has(sessionId) ? SESSION_EXITED_MESSAGE : SESSION_NOT_READY_MESSAGE;
  }

  function markSessionExited(sessionId: string) {
    if (exitedSessionsRef.current.has(sessionId)) return;
    const next = new Set(exitedSessionsRef.current);
    next.add(sessionId);
    exitedSessionsRef.current = next;
    setExitedSessions(next);
  }

  function forgetSession(sessionId: string) {
    terminalIoFailedRef.current.delete(sessionId);
    delete replayCharsRef.current[sessionId];
    delete altScreenRef.current[sessionId];
    delete altTailRef.current[sessionId];
    delete privateModesRef.current[sessionId];
    delete privateModeTailRef.current[sessionId];
    delete execSinceRef.current[sessionId];
    delete terminalWriteChainRef.current[sessionId];
    setAppNotes((prev) => {
      if (!(sessionId in prev)) return prev;
      const { [sessionId]: _removed, ...rest } = prev;
      return rest;
    });
    setStalledBoot((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Set(prev);
      next.delete(sessionId);
      return next;
    });
    setStuckForeground((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Set(prev);
      next.delete(sessionId);
      return next;
    });
    if (!exitedSessionsRef.current.has(sessionId)) return;
    const next = new Set(exitedSessionsRef.current);
    next.delete(sessionId);
    exitedSessionsRef.current = next;
    setExitedSessions(next);
  }

  /** Re-read session_list: the backend reports a dead shell as status "exited". */
  async function refreshSessionStatuses() {
    try {
      const res = await listSessions();
      for (const s of res.sessions ?? []) {
        if ((s.status as string) === "exited") markSessionExited(s.id);
      }
    } catch {
      // status refresh is best-effort
    }
  }

  /** Every exec-state change goes through here so the I/O failure latch and exit tracking stay in step. */
  function noteExecState(sessionId: string, state: SessionExecState) {
    setSessionExecState(sessionId, state);
    // The badge belongs to a command the app started. When the shell reports anything else
    // (a prompt, a hand-typed command, a desync) and no execute call is in flight, a leftover
    // "running" badge is stale and would refuse every submit.
    if (
      !execOwnsBadge(state) &&
      sessionBadgeRef.current[sessionId] === "running" &&
      !inFlightExecRef.current.has(sessionId)
    ) {
      publishBadge(sessionId, "idle");
    }
    if (state !== "booting") {
      setStalledBoot((prev) => {
        if (!prev.has(sessionId)) return prev;
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
    if (state === "ready" || state === "running" || state === "userRunning") {
      // The session is accepting input again: a later failure is a new failure.
      terminalIoFailedRef.current.delete(sessionId);
    }
    if (state === "desynced") {
      // A dead shell is reported as desynced; session_list says which it is.
      void refreshSessionStatuses();
    }
  }

  /** Adopt a backend session on boot, seeding its state instead of leaving it "booting". */
  function seedSessionState(s: SessionSummaryWithState) {
    if ((s.status as string) === "exited") {
      markSessionExited(s.id);
      setSessionExecState(s.id, "desynced");
      return;
    }
    const exec = EXEC_STATES.find((e) => e === s.execState);
    if (exec) {
      setSessionExecState(s.id, exec);
      // Only a command the app started has a badge; a hand-typed one has no finish event to lower it.
      if (execOwnsBadge(exec)) publishBadge(s.id, "running");
    } else if (!browserPreview) {
      // State unknown (older backend): offer Resync rather than a dead composer.
      setSessionExecState(s.id, "desynced");
    }
  }

  /** A line the app wrote itself. Shown beside the terminal, never in the xterm stream. */
  function appendAppNote(sessionId: string, text: string) {
    const line = text.replace(/[\r\n]+$/, "");
    if (line.length === 0) return;
    setAppNotes((prev) => {
      const list = [...(prev[sessionId] ?? []), line];
      return { ...prev, [sessionId]: list.length > APP_NOTES_MAX ? list.slice(-APP_NOTES_MAX) : list };
    });
  }

  /**
   * Adopt the backend's exec state for sessions this window still shows as booting (a
   * session:ready emitted before the listeners attached, or a missed event, would otherwise
   * leave a dead composer). Never overrides a state an event already delivered.
   */
  async function reconcileSessionStates() {
    try {
      const res = await listSessions();
      for (const s of res.sessions ?? []) {
        if ((s.status as string) === "exited") {
          markSessionExited(s.id);
          continue;
        }
        const exec = EXEC_STATES.find((e) => e === (s as SessionSummaryWithState).execState);
        const current = execStateOf(s.id);
        if (!exec || exec === "booting") continue;
        if (current === undefined || current === "booting") {
          setSessionExecState(s.id, exec);
          if (execOwnsBadge(exec)) publishBadge(s.id, "running");
        } else if (current === "userRunning" && exec !== "userRunning") {
          // A typed command that ended while no event reached this window (a webview
          // reload adopts the state, and the prompt that follows may predate the listeners).
          noteExecState(s.id, exec);
        }
      }
    } catch {
      // best-effort: the stall banner still offers Resync
    }
  }

  function appendTerminalLine(sessionId: string, line: string) {
    const runningExec = runningExecBySessionRef.current[sessionId];
    if (runningExec) {
      outputByExecRef.current[runningExec] = (outputByExecRef.current[runningExec] ?? "") + line;
    }
    // Every PTY chunk is kept and shown: a read of real output can begin with any text, so
    // reduced clutter must never drop one. (It applies to the app's own annotations.)

    // Store in background buffer: chunks are coalesced and the total is capped by size.
    const buffers = terminalLinesBySessionRef.current;
    const buffer = buffers[sessionId] ?? (buffers[sessionId] = []);
    const last = buffer.length - 1;
    if (last >= 0 && buffer[last].length < TERMINAL_REPLAY_CHUNK_CHARS && !isReplayPrefix(buffer[last])) {
      buffer[last] += line;
    } else {
      buffer.push(line);
    }
    // Track whether the app is in the alternate screen (vim, less, htop), scanning a short
    // tail too so a sequence split across two reads is still seen.
    const tail = altTailRef.current[sessionId] ?? "";
    const scanned = tail + line;
    altScreenRef.current[sessionId] = altScreenAfter(altScreenRef.current[sessionId] ?? false, scanned);
    altTailRef.current[sessionId] = scanned.slice(-ALT_SCREEN_TAIL_CHARS);
    // Same for the private modes xterm's reset would drop (bracketed paste, cursor keys, mouse).
    const modeScanned = (privateModeTailRef.current[sessionId] ?? "") + line;
    privateModesRef.current[sessionId] = privateModesAfter(
      privateModesRef.current[sessionId] ?? {},
      modeScanned,
    );
    privateModeTailRef.current[sessionId] = modeScanned.slice(-PRIVATE_MODE_TAIL_CHARS);

    let total = (replayCharsRef.current[sessionId] ?? 0) + line.length;
    if (total > TERMINAL_REPLAY_MAX_CHARS) {
      // Drop whole chunks from the front.
      while (buffer.length > 1 && total > TERMINAL_REPLAY_MAX_CHARS) {
        total -= buffer[0].length;
        buffer.shift();
      }
      const first = buffer[0];
      if (altScreenRef.current[sessionId]) {
        // Mid full-screen app: a main-screen marker or a line cut would replay the app's
        // cursor-addressed frames onto the shell screen. Re-enter the alternate screen
        // instead and let the app's next frames repaint it.
        if (first !== undefined && first !== TERMINAL_REPLAY_ALT_PREFIX) {
          buffer.unshift(TERMINAL_REPLAY_ALT_PREFIX);
          total += TERMINAL_REPLAY_ALT_PREFIX.length;
        }
      } else if (first !== undefined && first !== TERMINAL_REPLAY_TRUNCATED_MARKER) {
        // Cut the first kept chunk at a line boundary so replay never starts inside an
        // escape sequence or a line.
        const nl = first.indexOf("\n");
        if (nl >= 0) {
          total -= nl + 1;
          buffer[0] = first.slice(nl + 1);
        }
        buffer.unshift(TERMINAL_REPLAY_TRUNCATED_MARKER);
        total += TERMINAL_REPLAY_TRUNCATED_MARKER.length;
      }
    }
    replayCharsRef.current[sessionId] = total;

    // Write to terminal if this is the active session
    if (sessionId === activeSessionIdRef.current) {
      terminalPaneRef.current?.write(line);
    }
  }

  // --- Boot / hydration ---
  useEffect(() => {
    async function boot() {
      if (bootedRef.current) return;
      bootedRef.current = true;
      try {
        // Settings
        let settingsLoaded = false;
        try {
          const settingsRes = await settingsGet();
          settingsLoaded = true;
          if (settingsRes.settings) {
            const s = settingsRes.settings as Record<string, unknown>;
            if (typeof s.productMode === "string") setProductMode(s.productMode as "classic" | "guided");
            if (typeof s.defaultInputMode === "string") setDefaultInputMode(s.defaultInputMode as "command" | "ask");
            if (typeof s.fontSize === "string" && s.fontSize.trim()) setFontSize(s.fontSize);
            if (typeof s.simplifiedSummaries === "boolean") setSimplifiedSummaries(s.simplifiedSummaries);
            if (typeof s.plannerModel === "string" && s.plannerModel.trim()) setPlannerModel(s.plannerModel);
            if (typeof s.plannerEndpoint === "string" && s.plannerEndpoint.trim()) setPlannerEndpoint(s.plannerEndpoint);
          }
        } catch {
          // settings not critical — do not hydrate, or a later write would replace saved preferences
        }

        // Sessions
        let sessionsRes;
        try {
          sessionsRes = await listSessions();
        } catch {
          sessionsRes = null;
        }

        if (sessionsRes?.sessions?.length) {
          setSessions(sessionsRes.sessions);
          setActiveSessionId(sessionsRes.sessions[0].id);
          // A reloaded webview lost every event: seed state from the list itself.
          for (const adopted of sessionsRes.sessions) seedSessionState(adopted);
        } else {
          const res = await createSession({ label: "Session 1" });
          addSession(res.session);
          appendAppNote(
            res.session.id,
            `Welcome to CommandUI — ${escapeForTerminal(res.session.label)}\r\n`,
          );
          // The ready event may have fired before this window listened for it.
          void reconcileSessionStates();
        }

        // History
        try {
          const histRes = await historyList({ limit: 100 });
          if (histRes.items?.length) {
            loadHistory(histRes.items);
          }
        } catch {
          // history not critical
        }

        // Memory
        try {
          const memRes = await memoryList();
          setMemoryItems(memRes.items ?? []);
          setMemorySuggestions(memRes.suggestions ?? []);
          // Ids the database already settled; detectors must not re-propose them.
          const settled = memRes as typeof memRes & {
            dismissedSuggestionIds?: string[];
            acceptedSuggestionIds?: string[];
          };
          for (const id of [
            ...(settled.dismissedSuggestionIds ?? []),
            ...(settled.acceptedSuggestionIds ?? []),
          ]) {
            settledSuggestionIdsRef.current.add(id);
          }
        } catch {
          // memory not critical
        }
        try {
          // The dedicated command is the source of truth for settled ids (an older memory_list
          // does not carry them); a dismissed suggestion must not come back after a restart.
          const resolved = await memoryListResolvedSuggestions();
          for (const r of resolved.resolved ?? []) settledSuggestionIdsRef.current.add(r.id);
        } catch {
          // best-effort
        }

        // Run pattern detectors on boot
        try {
          await generateSuggestions();
        } catch {
          // suggestion generation not critical
        }

        // Workflows
        try {
          const wfRes = await workflowList();
          setWorkflows(wfRes.workflows ?? []);
        } catch {
          // workflows not critical
        }

        if (settingsLoaded) {
          settingsHydratedRef.current = true;
        }
        setBootPhase("ready");
      } catch (e: unknown) {
        const msg = errorText(e);
        setBootError(msg);
        setBootPhase("failed");
        setError(`CommandUI did not start. ${msg}`);
      }
    }
    boot();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // --- Sync input mode with settings ---
  useEffect(() => {
    setInputMode(defaultInputMode);
  }, [defaultInputMode, setInputMode]);

  function noteExecutionStarted(event: TerminalExecutionStartedEvent) {
    const sessionId = event.execution.sessionId;
    if (!sessionId) return;
    const executionId = event.execution.id;
    commandByExecRef.current[executionId] = event.execution.command ?? "";
    outputByExecRef.current[executionId] = "";
    runningExecBySessionRef.current[sessionId] = executionId;
    setRequestOffers((prev) => {
      if (!(sessionId in prev)) return prev;
      const next = { ...prev };
      delete next[sessionId];
      return next;
    });
    publishBadge(sessionId, "running");
    noteExecState(sessionId, "running");
    if (sessionId === activeSessionIdRef.current) {
      setActiveExecution(executionId);
    }
  }

  function noteExecutionFinished(event: TerminalExecutionFinishedEvent) {
    const status = event.status;
    const badge: SessionBadgeStatus =
      status === "failure" ? "failure" : status === "success" ? "success" : "idle";
    const output = outputByExecRef.current[event.executionId] ?? "";
    const command =
      commandByExecRef.current[event.executionId] ??
      useHistoryStore.getState().items.find((item) => item.id === event.executionId)?.executedCommand ??
      "";
    delete outputByExecRef.current[event.executionId];
    delete commandByExecRef.current[event.executionId];
    if (event.sessionId && runningExecBySessionRef.current[event.sessionId] === event.executionId) {
      delete runningExecBySessionRef.current[event.sessionId];
    }
    const exitKnown =
      event.exitKnown !== false &&
      event.status !== "unknown" &&
      event.reason !== "exit_unknown" &&
      event.reason !== "shell_exited" &&
      event.reason !== "input_not_accepted";
    const phase =
      event.status === "success"
        ? "success"
        : event.status === "interrupted"
          ? "interrupted"
          : event.status === "unknown" || event.reason === "exit_unknown"
            ? "unknown"
            : "failure";
    const visible = collapseRedraws(output, command);
    const view = describeResult({
      phase,
      exitCode: event.exitCode,
      exitKnown: event.exitKnown,
      reason: event.reason,
      outputText: visible,
      outputLines: countOutputLines(visible),
      command,
    });
    if (event.sessionId) {
      publishBadge(event.sessionId, badge);
      const exec = useExecutionStore.getState().sessionExecStates[event.sessionId];
      if (exec === "running" || exec === "interrupting") {
        setSessionExecState(event.sessionId, "ready");
      }
      if (event.sessionId === activeSessionIdRef.current) {
        setActiveExecution(null);
      }
      const sessionId = event.sessionId;
      setResultsBySession((prev) => ({
        ...prev,
        [sessionId]: {
          command,
          view,
          output: visible,
          outputOpen: false,
          exitCode: exitKnown ? event.exitCode : null,
          exitKnown,
        },
      }));
      setOutputBlocksBySession((prev) => {
        const list = prev[sessionId] ?? [];
        const block: OutputBlock = {
          id: event.executionId,
          command,
          headline: resultText(view),
          output: visible,
        };
        const without = list.filter((item) => item.id !== block.id);
        return { ...prev, [sessionId]: [...without, block].slice(-40) };
      });
    }
    setLastExecutionId(event.executionId);

    const historyId =
      executionToHistoryRef.current[event.executionId] ?? event.executionId;
    const finishedAt = new Date().toISOString();
    const historyItem = useHistoryStore.getState().items.find((h) => h.id === historyId);
    const durationMs = historyItem
      ? Date.now() - new Date(historyItem.createdAt).getTime()
      : undefined;

    updateHistoryItem(historyId, {
      status,
      exitCode: exitKnown ? event.exitCode : undefined,
      finishedAt,
      durationMs,
    });
    persistInBackground("history update", historyUpdate({
      historyId,
      status,
      exitCode: exitKnown ? event.exitCode : undefined,
      finishedAt,
      durationMs,
    }));
    void generateSuggestions();
  }

  // Drop in-flight workflow polls if this shell unmounts (including StrictMode's remount).
  useEffect(() => {
    const controllers = workflowAbortBySessionRef.current;
    return () => {
      for (const controller of controllers.values()) controller.abort();
      controllers.clear();
    };
  }, []);

  // --- Terminal event subscriptions ---
  useEffect(() => {
    let disposed = false;
    const pending: Array<Promise<() => void>> = [];

    const remember = (subscription: Promise<() => void>) => {
      const tracked = subscription.then(
        (unlisten) => {
          if (disposed) {
            unlisten();
            return () => {};
          }
          return unlisten;
        },
        () => () => {},
      );
      pending.push(tracked);
    };

    if (browserPreview) {
      const unlisteners = [
        onMockEvent<{ sessionId: string; executionId?: string; text: string }>(
          "terminal:line",
          (event) => appendTerminalLine(event.sessionId, event.text),
        ),
        onMockEvent<TerminalExecutionStartedEvent>(
          "terminal:execution_started",
          (event) => noteExecutionStarted(event),
        ),
        onMockEvent<TerminalExecutionFinishedEvent>(
          "terminal:execution_finished",
          (event) => noteExecutionFinished(event),
        ),
        onMockEvent<{ sessionId: string; cwd: string }>(
          "session:ready",
          (event) => {
            noteExecState(event.sessionId, "ready");
          },
        ),
        onMockEvent<{ sessionId: string; execState: SessionExecState }>(
          "session:exec_state_changed",
          (event) => {
            noteExecState(event.sessionId, event.execState);
          },
        ),
      ];
      return () => {
        disposed = true;
        for (const unlisten of unlisteners) unlisten();
      };
    }

    remember(subscribeToTerminalLines((event) => {
      appendTerminalLine(event.sessionId, event.text);
    }));
    remember(subscribeToExecutionStarted((event) => {
      noteExecutionStarted(event);
    }));
    remember(subscribeToExecutionFinished((event) => {
      noteExecutionFinished(event);
    }));
    remember(subscribeToSessionCwdChanged((event) => {
      updateSession(event.sessionId, { cwd: event.cwd });
    }));
    remember(subscribeToSessionReady((event) => {
      noteExecState(event.sessionId, "ready");
    }));
    remember(subscribeToExecStateChanged((event) => {
      noteExecState(event.sessionId, event.execState);
    }));
    // Listeners are live: adopt any state that changed before they attached.
    void Promise.all(pending)
      .then(() => {
        if (!disposed) void reconcileSessionStates();
      })
      .catch((e: unknown) => {
        const msg = errorText(e);
        console.error("[AppShell] Listener setup failed:", msg);
        setError(`CommandUI could not finish starting. ${msg} Some parts may not work until you close the window and open CommandUI again.`);
      });

    return () => {
      disposed = true;
      for (const subscription of pending) {
        void subscription
          .then((unlisten) => {
            unlisten();
          })
          .catch((e: unknown) => {
            const msg = errorText(e);
            console.warn("[AppShell] Unsubscribe failed:", msg);
          });
      }
    };
    // Subscriptions live for the whole shell. Resubscribing is asynchronous, so events in the gap would be lost.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // --- Replay buffer on session switch ---
  useEffect(() => {
    if (!activeSessionId) return;
    const pane = terminalPaneRef.current;
    if (!pane) return;

    // replay() clears the terminal and drops xterm's own query replies while the stored
    // stream is parsed, so a stale terminal query in it cannot type a reply into the live
    // shell. The modes the shell last set come first: a truncated or cleared buffer no
    // longer carries the sequences that set them, and reset turned them off.
    const stored = terminalLinesBySessionRef.current[activeSessionId] ?? [];
    const modes = privateModePrefix(privateModesRef.current[activeSessionId]);
    pane.replay(modes ? [modes, ...stored] : stored);
    // A hand-typed command may have ended while this tab was in the background.
    if (execStateOf(activeSessionId) === "userRunning") void reconcileSessionStates();
  }, [activeSessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  // --- Note when every session entered its exec state ---
  // Stall detection reads these times, so switching tabs never restarts a wait.
  useEffect(() => {
    const since = execSinceRef.current;
    const live = new Set<string>();
    for (const s of sessions) {
      live.add(s.id);
      const state = sessionExecStates[s.id] ?? "booting";
      const known = since[s.id];
      if (known?.state === state) continue;
      since[s.id] = { state, at: Date.now() };
      if (!execIsForeground(state)) {
        setStuckForeground((prev) => {
          if (!prev.has(s.id)) return prev;
          const next = new Set(prev);
          next.delete(s.id);
          return next;
        });
      }
    }
    for (const sid of Object.keys(since)) {
      if (!live.has(sid)) delete since[sid];
    }
  }, [sessions, sessionExecStates]);

  // --- One watcher over every session: a stalled boot or a long typed command gets a way out ---
  useEffect(() => {
    if (browserPreview) return;
    const timer = setInterval(() => {
      const now = Date.now();
      for (const [sid, { state, at }] of Object.entries(execSinceRef.current)) {
        if (exitedSessionsRef.current.has(sid)) continue;
        if (state === "booting" && now - at >= BOOT_STALL_MS && !stalledBootRef.current.has(sid)) {
          void reconcileSessionStates()
            .then(() => {
              if ((execStateOf(sid) ?? "booting") !== "booting") return;
              setStalledBoot((prev) => (prev.has(sid) ? prev : new Set(prev).add(sid)));
            })
            .catch((e: unknown) => {
              const msg = errorText(e);
              console.error("[AppShell] Reconcile session states failed during stall check:", msg);
              setError(`CommandUI could not check this session (${msg}). Choose Resync if a session looks stuck.`);
            });
        } else if (
          execIsForeground(state) &&
          now - at >= FOREGROUND_STUCK_MS &&
          !stuckForegroundRef.current.has(sid)
        ) {
          setStuckForeground((prev) => (prev.has(sid) ? prev : new Set(prev).add(sid)));
        }
      }
    }, SESSION_WATCH_MS);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [browserPreview]);

  // --- Settings persistence ---
  useEffect(() => {
    if (browserPreview) return;
    if (!settingsHydratedRef.current) return;
    persistInBackground("settings update", settingsUpdate({
      settings: {
        productMode,
        fontSize,
        simplifiedSummaries,
        plannerModel,
        plannerEndpoint,
        defaultInputMode,
      },
    }));
  }, [browserPreview, productMode, fontSize, simplifiedSummaries, plannerModel, plannerEndpoint, defaultInputMode]);

  // --- Background persistence failure banner ---
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as {
        what: string;
        message: string;
      };
      setError(`CommandUI could not save your latest change: ${detail.message}. Try again. If this keeps happening, the change may be lost when you close the window.`);
    };
    window.addEventListener("commandui:persist-failed", handler);
    return () => window.removeEventListener("commandui:persist-failed", handler);
  }, []);

  function cancelConfirm() {
    const resolve = confirmResolveRef.current;
    confirmResolveRef.current = null;
    setPendingConfirm(null);
    resolve?.(false);
  }

  function askConfirm(choice: Omit<ConfirmRequest, "onConfirm">): Promise<boolean> {
    cancelConfirm();
    return new Promise((resolve) => {
      confirmResolveRef.current = resolve;
      setPendingConfirm({
        ...choice,
        onConfirm: () => {
          confirmResolveRef.current = null;
          setPendingConfirm(null);
          resolve(true);
        },
      });
    });
  }

  function clearUndoTimer() {
    if (undoTimerRef.current !== null) {
      window.clearTimeout(undoTimerRef.current);
      undoTimerRef.current = null;
    }
  }

  function armUndo(next: UndoRequest) {
    const previous = pendingUndoRef.current;
    if (previous && previous.token !== next.token) previous.commit();
    pendingUndoRef.current = next;
    setUndoMessage(next.message);
    clearUndoTimer();
    undoTimerRef.current = window.setTimeout(() => {
      if (pendingUndoRef.current?.token !== next.token) return;
      pendingUndoRef.current = null;
      undoTimerRef.current = null;
      setUndoMessage(null);
      next.commit();
    }, UNDO_MS);
  }

  function undoPending() {
    const current = pendingUndoRef.current;
    if (!current) return;
    pendingUndoRef.current = null;
    clearUndoTimer();
    setUndoMessage(null);
    current.restore();
  }

  useEffect(() => {
    return () => {
      clearUndoTimer();
      const current = pendingUndoRef.current;
      pendingUndoRef.current = null;
      current?.commit();
    };
  }, []);

  // --- Centralized keyboard shortcuts ---
  const overlayRef = useRef({
    historyOpen: false,
    workflowOpen: false,
    memoryOpen: false,
    settingsOpen: false,
    paletteOpen: false,
    helpOpen: false,
    outputOpen: false,
    editorOpen: false,
    planOpen: false,
  });
  overlayRef.current = {
    historyOpen,
    workflowOpen,
    memoryOpen,
    settingsOpen,
    paletteOpen,
    helpOpen,
    outputOpen,
    editorOpen: workflowEditorData !== null,
    planOpen: plan !== null,
  };

  function setOverlay(name: "history" | "workflow" | "memory" | "settings" | "palette" | "help" | "output" | null) {
    setHistoryOpen(name === "history");
    setWorkflowOpen(name === "workflow");
    setMemoryOpen(name === "memory");
    setSettingsOpen(name === "settings");
    setPaletteOpen(name === "palette");
    setHelpOpen(name === "help");
    setOutputOpen(name === "output");
  }

  function toggleOverlay(name: "history" | "workflow" | "memory" | "settings" | "palette" | "help" | "output") {
    const overlay = overlayRef.current;
    const open =
      (name === "history" && overlay.historyOpen) ||
      (name === "workflow" && overlay.workflowOpen) ||
      (name === "memory" && overlay.memoryOpen) ||
      (name === "settings" && overlay.settingsOpen) ||
      (name === "palette" && overlay.paletteOpen) ||
      (name === "help" && overlay.helpOpen) ||
      (name === "output" && overlay.outputOpen);
    setOverlay(open ? null : name);
  }

  function closeAllOverlays() {
    const overlay = overlayRef.current;
    if (overlay.editorOpen) { setWorkflowEditorData(null); return; }
    if (
      overlay.paletteOpen ||
      overlay.helpOpen ||
      overlay.outputOpen ||
      overlay.historyOpen ||
      overlay.workflowOpen ||
      overlay.memoryOpen ||
      overlay.settingsOpen
    ) {
      setOverlay(null);
      requestAnimationFrame(() => {
        restorePreviousZone();
        composerRef.current?.focus();
      });
    }
    // Escape never rejects a plan. Reject stays on the plan's R key and its button.
  }

  function closeWelcome() {
    setWelcomeOpen(false);
    requestAnimationFrame(() => composerRef.current?.focus());
  }

  function changeShowWelcome(show: boolean) {
    setShowWelcomeAtStartup(show);
    writeShowWelcome(show);
  }

  function focusComposer() {
    composerRef.current?.focus();
  }

  const shortcuts = useMemo<ShortcutDef[]>(() => {
    const defs: ShortcutDef[] = [
      { id: "palette",       combo: "ctrl+k",       context: ["global"], action: () => setOverlay("palette") },
      // Ctrl+Shift variants: the only app chords that work while the terminal has focus
      // (plain Ctrl+<letter> goes to the shell there).
      { id: "palette-term",  combo: "ctrl+shift+k", context: ["global"], action: () => setOverlay("palette") },
      { id: "focus-composer-term", combo: "ctrl+shift+j", context: ["global"], action: focusComposer },
      { id: "clear-terminal-term", combo: "ctrl+shift+l", context: ["global"], action: clearTerminalView },
      { id: "new-session-term", combo: "ctrl+shift+t", context: ["global"], action: handleCreateSession },
      { id: "history-term",   combo: "ctrl+shift+h", context: ["global"], action: () => toggleOverlay("history") },
      { id: "memory-term",    combo: "ctrl+shift+m", context: ["global"], action: () => toggleOverlay("memory") },
      { id: "focus-composer", combo: "ctrl+j",       context: ["global"], action: focusComposer },
      { id: "clear-terminal", combo: "ctrl+l",       context: ["global"], action: clearTerminalView },
      { id: "new-session",   combo: "ctrl+t",        context: ["global"], action: handleCreateSession },
      { id: "history",       combo: "ctrl+h",        context: ["global"], action: () => toggleOverlay("history") },
      { id: "workflows",     combo: "ctrl+shift+w",  context: ["global"], action: () => toggleOverlay("workflow") },
      { id: "memory",        combo: "ctrl+m",        context: ["global"], action: () => toggleOverlay("memory") },
      { id: "settings",      combo: "ctrl+,",        context: ["global"], action: () => toggleOverlay("settings") },
      { id: "help",          combo: "f1",           context: ["global"], action: () => toggleOverlay("help") },
      { id: "output",        combo: "ctrl+shift+o", context: ["global"], action: () => toggleOverlay("output") },
      { id: "toggle-mode",   combo: "ctrl+shift+a", context: ["global"], action: () => {
        const mode = useComposerStore.getState().inputMode;
        setInputMode(mode === "command" ? "ask" : "command");
      } },
      { id: "escape",        combo: "escape",        context: ["global"], action: closeAllOverlays },
      // Plan shortcuts. Bare keys do not fire in text fields (see resolveShortcut).
      // Approve uses the edited textarea command, and handleApprovePlan applies canRun.
      { id: "plan-approve",  combo: "a",             context: ["plan"],   when: () => plan !== null, action: () => approvePlanRef.current(planGateRef.current.command) },
      { id: "plan-approve-global", combo: "ctrl+enter", context: ["global"], when: () => plan !== null && useFocusStore.getState().currentZone !== "terminal", action: () => approvePlanRef.current(planGateRef.current.command) },
      { id: "plan-reject",   combo: "r",             context: ["plan"],   when: () => plan !== null, action: handleRejectPlan },
      { id: "plan-edit",     combo: "e",             context: ["plan"],   when: () => plan !== null, action: () => {
        // Focus the command textarea in PlanPanel
        const el = document.querySelector(".plan-command-input") as HTMLTextAreaElement | null;
        el?.focus();
      }},
      // Session jump: Ctrl+1..9
      ...sessions.slice(0, 9).map((s, i) => ({
        id: `session-${i + 1}`,
        combo: `ctrl+${i + 1}`,
        context: ["global"] as ShortcutContext[],
        action: () => setActiveSessionId(s.id),
      })),
    ];

    // Close session only in Tauri mode (Ctrl+W conflicts with browser tab close).
    // Ctrl+W is the shell's word-rubout, so it never fires from the terminal zone
    // (see resolveShortcut); Ctrl+Shift+X is the chord that works everywhere.
    // Either one asks first when a command is running.
    if (!browserPreview) {
      const closeActive = () => {
        if (!activeSessionId) return;
        requestCloseSession(activeSessionId);
      };
      defs.push(
        { id: "close-session", combo: "ctrl+w", context: ["global"], action: closeActive },
        { id: "close-session-term", combo: "ctrl+shift+x", context: ["global"], action: closeActive },
      );
    }

    return defs;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    sessions,
    activeSessionId,
    plan,
    browserPreview,
    historyOpen,
    workflowOpen,
    memoryOpen,
    settingsOpen,
    paletteOpen,
    workflowEditorData,
  ]);

  useShortcuts(shortcuts);

  // --- Command palette actions ---
  const paletteActions = useMemo<PaletteAction[]>(() => {
    const actions: PaletteAction[] = [
      { id: "new-session",    label: "New Session",        shortcut: "Ctrl+T",       action: handleCreateSession },
      { id: "focus-composer",  label: "Focus Composer",    shortcut: "Ctrl+J",       action: focusComposer },
      { id: "clear-terminal",  label: "Clear Terminal",    shortcut: "Ctrl+L",       action: clearTerminalView },
      { id: "open-history",    label: "Open History",      shortcut: "Ctrl+H",       action: () => setOverlay("history") },
      { id: "open-workflows",  label: "Open Workflows",   shortcut: "Ctrl+Shift+W", action: () => setOverlay("workflow") },
      { id: "open-memory",     label: "Open Memory",      shortcut: "Ctrl+M",       action: () => setOverlay("memory") },
      { id: "open-settings",   label: "Open Settings",    shortcut: "Ctrl+,",       action: () => setOverlay("settings") },
      { id: "open-output",     label: "Open Output",      shortcut: "Ctrl+Shift+O", action: () => setOverlay("output") },
      { id: "open-help",       label: "Keyboard help",    shortcut: "F1",           action: () => setOverlay("help") },
      ...sessions.map((s, i) => ({
        id: `switch-session-${s.id}`,
        label: `Switch to ${s.label ?? `Session ${i + 1}`}`,
        shortcut: i < 9 ? `Ctrl+${i + 1}` : undefined,
        action: () => setActiveSessionId(s.id),
      })),
    ];

    if (isRunning) {
      actions.push({ id: "interrupt", label: "Stop the command", action: handleInterrupt });
    }
    // A dead shell cannot be resynced; New Session (above) is its way forward.
    if ((activeExecState === "desynced" || activeBootStalled) && !activeExited) {
      actions.push({ id: "resync", label: "Resync Terminal", action: handleResync });
    }
    if ((activeBootStalled || activeForegroundStuck) && activeSessionId) {
      const stuckId = activeSessionId;
      actions.push({ id: "close-stuck-session", label: "Close Session", action: () => requestCloseSession(stuckId) });
    }

    return actions;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessions, isRunning, activeExecState, activeExited, activeBootStalled, activeForegroundStuck]);

  // execute() rejected before any ExecutionFinished event: close the row out.
  function failRejectedExecution(historyId: string, err: unknown) {
    const finishedAt = new Date().toISOString();
    updateHistoryItem(historyId, { status: "failure", finishedAt });
    persistInBackground("history update", historyUpdate({ historyId, status: "failure", finishedAt }));
    void err;
  }

  // --- Submit handler ---
  // Resolves false when the submit was rejected, so the composer keeps the user's text.
  async function handleSubmit(value: string, modeOverride?: "command" | "ask"): Promise<boolean> {
    const mode = modeOverride ?? inputMode;
    if (!session) return false;
    if (busySessionsRef.current.has(session.id)) return false;
    if (mode === "command" && sessionIsRunning(session.id)) {
      setError(busyMessage(session.id));
      return false;
    }
    const sessionId = session.id;
    if (mode === "command" && sessionNotReady(sessionId)) {
      setError(notReadyMessage(sessionId));
      return false;
    }
    if (mode === "command") {
      // Checked before any history row exists: a command the backend would refuse
      // (multi-line, control character) or that hides text must not leave a failure row.
      const problem = commandProblem(value);
      if (problem) {
        setError(problem);
        return false;
      }
      if (looksLikeRequest(value)) {
        setRequestOffers((prev) => ({ ...prev, [sessionId]: value }));
        setError(null);
        return false;
      }
    }
    setRequestOffers((prev) => {
      if (!(sessionId in prev)) return prev;
      const next = { ...prev };
      delete next[sessionId];
      return next;
    });
    let accepted = true;
    lockSession(sessionId);
    setError(null);

    try {
      if (mode === "command") {
        // --- Raw command flow ---
        if (sessionIsRunning(sessionId)) {
          setError(busyMessage(sessionId));
          accepted = false;
          return false;
        }
        inFlightExecRef.current.add(sessionId);
        const executionId = crypto.randomUUID();
        const historyItem: HistoryItem = {
          id: executionId,
          sessionId,
          source: "raw",
          userInput: value,
          executedCommand: value,
          status: "planned",
          createdAt: new Date().toISOString(),
          cwd: session.cwd,
        };
        appendHistoryItem(historyItem);
        executionToHistoryRef.current[executionId] = executionId;

        persistInBackground("history append", historyAppend({ item: historyItem }));

        try {
          await executeCommand({
            executionId,
            sessionId,
            command: value,
            source: "raw",
          });
        } catch (err) {
          failRejectedExecution(historyItem.id, err);
          throw err;
        } finally {
          inFlightExecRef.current.delete(sessionId);
        }
      } else {
        // --- Semantic flow ---
        const historyId = crypto.randomUUID();

        const context = buildPlannerContext({
          sessionId: session.id,
          cwd: session.cwd ?? ".",
          shell: session.shell ?? "unknown",
          os: detectOS(),
          memoryItems,
          workflows,
          lastRunByWorkflowId,
          recentHistory: visibleHistoryItems,
        });

        const res = await generatePlan({
          sessionId: session.id,
          userIntent: value,
          context,
          model: plannerModel,
          endpoint: plannerEndpoint,
        });

        // The user switched tabs while the plan was generating: drop it rather than show
        // a plan for another session that Approve could run in the wrong place.
        if (activeSessionIdRef.current !== sessionId) return true;

        setPlannerStatus(res.status);
        if (!res.plan || !res.review) {
          setPlan(null);
          setPlanNotice(null);
          return true;
        }

        planGateRef.current = { command: "", confirmed: false };
        setPlanNotice(null);
        setPlan({ plan: res.plan, review: res.review, status: res.status });
        setPlanNonce((n) => n + 1);
        setCurrentPlanHistoryId(historyId);

        const historyItem: HistoryItem = {
          id: historyId,
          sessionId: session.id,
          source: "semantic",
          userInput: value,
          generatedCommand: res.plan.command,
          linkedPlanId: res.plan.id,
          status: "planned",
          createdAt: new Date().toISOString(),
          cwd: session.cwd,
          plannerSource: recordedPlannerSource(String(res.plan.source)),
        };
        appendHistoryItem(historyItem);

        persistInBackground("history append", historyAppend({ item: historyItem }));
        persistInBackground("plan store", planStore({ plan: res.plan }));

        // Lines the app writes itself are escaped: model text must not move the cursor
        // or hide characters in the transcript.
        appendAppNote(session.id, `? ${escapeForTerminal(value)}\r\n`);
        appendAppNote(session.id, `Drafted: ${escapeForTerminal(res.plan.command)}\r\n`);
      }
    } catch (e: unknown) {
      const msg = errorText(e);
      setError(msg);
      accepted = false;
    } finally {
      unlockSession(sessionId);
    }
    return accepted;
  }

  // --- Plan actions ---
  async function handleApprovePlan(approvedCommand: string) {
    if (!plan) return;
    // Execute on the session the plan was made for, never on whichever tab is active now.
    const planSessionId = plan.plan.sessionId;
    const session = sessions.find((s) => s.id === planSessionId) ?? null;
    if (!session) {
      setError("The session this plan was made for is closed. Run it in the current session instead, or reject the plan.");
      return;
    }
    if (planSessionId !== activeSessionIdRef.current) {
      setError(`This plan runs in "${session.label}", which is not the session you are viewing. Go to that session, or run the plan here instead.`);
      return;
    }
    if (approveInFlightRef.current) return;
    if (sessionIsRunning(session.id) || busySessionsRef.current.has(session.id)) {
      setError(busyMessage(session.id));
      return;
    }
    if (sessionNotReady(session.id)) {
      setError(notReadyMessage(session.id));
      return;
    }
    const trimmed = approvedCommand.trim();
    const gate = planGateRef.current;
    // Same rule as PlanPanel's Run button. Shortcuts pass the edited command
    // the panel reported; a different string (the original plan command) does not run.
    if (trimmed.length === 0 || trimmed !== gate.command) return;
    if (
      !planCanRun({
        command: trimmed,
        risk: plan.plan.risk,
        confirmed: gate.confirmed,
        flags: plan.plan,
      })
    ) {
      return;
    }
    // Before any row or "[approved]" line exists: a command the backend would refuse must
    // not be recorded or announced as approved.
    const problem = commandProblem(trimmed);
    if (problem) {
      setError(problem);
      return;
    }
    const sessionId = session.id;
    lockSession(sessionId);
    approveInFlightRef.current = true;

    try {
      if (sessionIsRunning(sessionId)) {
        setError(busyMessage(sessionId));
        return;
      }
      inFlightExecRef.current.add(sessionId);
      const executionId = crypto.randomUUID();

      // A plan opened from history has no live row (currentPlanHistoryId is
      // null): the run gets a NEW row and the original outcome is untouched.
      let runHistoryId: string;
      let createdRunRow = false;
      const planRow = currentPlanHistoryId
        ? useHistoryStore.getState().items.find((h) => h.id === currentPlanHistoryId)
        : undefined;
      // A plan moved to another session gets its own row, so history files it under the session it ran in.
      if (currentPlanHistoryId && (!planRow || planRow.sessionId === sessionId)) {
        runHistoryId = currentPlanHistoryId;
        // Local only until execute accepts; the database is written after success.
        updateHistoryItem(runHistoryId, { executedCommand: trimmed });
      } else {
        runHistoryId = executionId;
        createdRunRow = true;
        const newRow: HistoryItem = {
          id: runHistoryId,
          sessionId,
          source: "semantic",
          userInput: plan.plan.userIntent,
          generatedCommand: plan.plan.command,
          executedCommand: trimmed,
          linkedPlanId: plan.plan.id,
          status: "planned",
          createdAt: new Date().toISOString(),
          cwd: session.cwd,
        };
        appendHistoryItem(newRow);
        persistInBackground("history append", historyAppend({ item: newRow }));
      }
      executionToHistoryRef.current[executionId] = runHistoryId;

      // Check for edit-based memory suggestion (persisted after execute accepts)
      let pendingSuggestion: MemorySuggestion | null = null;
      if (
        trimmed !== plan.plan.command &&
        session.cwd
      ) {
        const existing = memorySuggestions.find(
          (s) =>
            s.kind === "accepted_substitution" &&
            s.proposedValue === trimmed &&
            s.projectRoot === session.cwd,
        );

        if (!existing) {
          const suggestion: MemorySuggestion = {
            id: crypto.randomUUID(),
            scope: "project",
            projectRoot: session.cwd,
            kind: "accepted_substitution",
            label: `Use "${trimmed}" instead of "${plan.plan.command}"`,
            proposedKey: plan.plan.command,
            proposedValue: trimmed,
            confidence: 0.72,
            derivedFromHistoryIds: [runHistoryId],
            status: "pending",
            createdAt: new Date().toISOString(),
          };
          pendingSuggestion = suggestion;
        }
      }

      appendAppNote(session.id, `Approved: ${escapeForTerminal(trimmed)}\r\n`);

      try {
        await executeCommand({
          executionId,
          sessionId,
          command: trimmed,
          source: "semantic",
          linkedPlanId: plan.plan.id,
        });
      } catch (err) {
        if (createdRunRow) {
          failRejectedExecution(runHistoryId, err);
        } else {
          // Row existed before the run: record the failure, drop the claim it ran.
          const finishedAt = new Date().toISOString();
          // executedCommand was never persisted for this row, so the database agrees.
          updateHistoryItem(runHistoryId, { status: "failure", executedCommand: undefined, finishedAt });
          persistInBackground("history update", historyUpdate({ historyId: runHistoryId, status: "failure", finishedAt }));
        }
        throw err;
      } finally {
        inFlightExecRef.current.delete(sessionId);
      }

      if (!createdRunRow) {
        persistInBackground("history update", historyUpdate({ historyId: runHistoryId, executedCommand: trimmed }));
      }

      if (pendingSuggestion) {
        const toStore: MemorySuggestion = pendingSuggestion;
        try {
          await memoryStoreSuggestion({ suggestion: toStore });
          setMemorySuggestions((prev) => [toStore, ...prev]);
        } catch (storeErr) {
          setError(
            `Could not save the memory suggestion: ${errorText(storeErr)}`,
          );
        }
      }

      setPlan(null);
      setPlanNotice(null);
      setCurrentPlanHistoryId(null);
    } catch (e: unknown) {
      const msg = errorText(e);
      setError(msg);
    } finally {
      approveInFlightRef.current = false;
      unlockSession(sessionId);
    }
  }

  approvePlanRef.current = (command: string) => {
    void handleApprovePlan(command);
  };

  function handleRejectPlan() {
    // An approve is awaiting execute for this plan: rejecting now would mark a running command rejected.
    const planSessionId = plan?.plan.sessionId;
    if (approveInFlightRef.current) return;
    if (planSessionId && busySessionsRef.current.has(planSessionId)) return;

    if (currentPlanHistoryId) {
      updateHistoryItem(currentPlanHistoryId, { status: "rejected" });
      persistInBackground("history update", historyUpdate({
        historyId: currentPlanHistoryId,
        status: "rejected",
      }));
    }

    // Works with no session open, or when the plan's session is gone.
    const noteSessionId =
      planSessionId && sessions.some((s) => s.id === planSessionId) ? planSessionId : session?.id;
    if (noteSessionId) appendAppNote(noteSessionId, "Rejected the draft.\r\n");
    setPlan(null);
    setPlanNotice(null);
    setCurrentPlanHistoryId(null);
  }

  /** Move the open plan to the session the user is viewing (a different cwd: confirm again). */
  function handleRetargetPlan() {
    if (!plan) return;
    const target = useSessionStore.getState().activeSessionId;
    if (!target || exitedSessionsRef.current.has(target)) return;
    setPlan({ ...plan, plan: { ...plan.plan, sessionId: target } });
    setPlanNotice(null);
  }

  function handleGoToPlanSession() {
    if (!plan) return;
    if (sessions.some((s) => s.id === plan.plan.sessionId)) {
      setActiveSessionId(plan.plan.sessionId);
    }
  }

  async function handleSaveWorkflow(command: string) {
    if (!session || !plan) return;

    const workflow: Workflow = {
      id: crypto.randomUUID(),
      label: plan.plan.userIntent.slice(0, 48),
      source: "semantic",
      originalIntent: plan.plan.userIntent,
      command,
      steps: [{ command }],
      projectRoot: session.cwd,
      createdAt: new Date().toISOString(),
    };

    // Show the workflow only once it is stored, or it would be undeletable.
    try {
      await workflowAdd({ workflow });
    } catch (e: unknown) {
      setError(`Could not save the workflow: ${errorText(e)}`);
      return;
    }
    addWorkflow(workflow);
    appendAppNote(session.id, `Saved workflow ${escapeForTerminal(workflow.label)}.\r\n`);
  }

  // --- Terminal handlers ---
  // A dead or not-ready session rejects every write and resize. Log once per
  // session and surface a single banner instead of one unhandled rejection per keystroke.
  // The latch clears when a write succeeds or the session reports ready/running again, and
  // a resize failure (it can race session start) is logged but never shown as "ended".
  function reportTerminalIoFailure(
    sessionId: string,
    write: Promise<unknown>,
    kind: "write" | "resize",
  ) {
    write.then(
      () => {
        if (kind === "write") terminalIoFailedRef.current.delete(sessionId);
      },
      (e: unknown) => {
        const msg = errorText(e);
        if (kind === "resize") {
          console.warn("[terminal] resize failed:", msg);
          return;
        }
        if (terminalIoFailedRef.current.has(sessionId)) return;
        terminalIoFailedRef.current.add(sessionId);
        console.warn("[terminal] write failed:", msg);
        // An exited shell already has its own banner; do not stack a second explanation.
        if (exitedSessionsRef.current.has(sessionId)) return;
        setError(`This terminal session has ended or is not accepting input (${msg}).`);
      },
    );
  }

  const handleTerminalData = useCallback(
    (data: string) => {
      if (!activeSessionId) return;
      // terminal_write is an async command, so concurrent invokes can reach the shell out
      // of order. Each write starts only after the previous one for this session settled.
      const chain = terminalWriteChainRef.current;
      const send = (chain[activeSessionId] ?? Promise.resolve())
        .catch(() => undefined)
        .then(() => writeTerminal({ sessionId: activeSessionId, data }));
      chain[activeSessionId] = send;
      reportTerminalIoFailure(activeSessionId, send, "write");
    },
    [activeSessionId],
  );

  const handleTerminalResize = useCallback(
    (cols: number, rows: number) => {
      if (!activeSessionId) return;
      reportTerminalIoFailure(activeSessionId, resizeTerminal({ sessionId: activeSessionId, cols, rows }), "resize");
    },
    [activeSessionId],
  );

  // --- Interrupt handler ---
  async function handleInterrupt() {
    const sessionId = useSessionStore.getState().activeSessionId;
    if (!sessionId || !sessionIsRunning(sessionId)) return;
    // Already interrupting: a second press has nothing to add and the backend would refuse it.
    if (execStateOf(sessionId) === "interrupting") return;
    try {
      await interruptTerminal({ sessionId });
    } catch (e: unknown) {
      const msg = errorText(e);
      // The command ended between the press and the call: nothing left to interrupt.
      if (/no command is currently running/i.test(msg)) return;
      setError(msg);
    }
  }

  // --- Resync handler ---
  async function handleResync() {
    if (!activeSessionId) return;
    // Re-arm the boot timer: a resync restarts the wait for the ready marker.
    setStalledBoot((prev) => {
      if (!prev.has(activeSessionId)) return prev;
      const next = new Set(prev);
      next.delete(activeSessionId);
      return next;
    });
    const known = execSinceRef.current[activeSessionId];
    if (known) execSinceRef.current[activeSessionId] = { state: known.state, at: Date.now() };
    try {
      await resyncTerminal({ sessionId: activeSessionId });
    } catch (e: unknown) {
      if (isSessionExitedError(e)) {
        markSessionExited(activeSessionId);
        return;
      }
      setError(errorText(e));
    }
  }

  // --- Session handlers ---
  async function handleCreateSession() {
    try {
      const label = `Session ${sessions.length + 1}`;
      const res = await createSession({ label });
      addSession(res.session);
      setActiveSessionId(res.session.id);
      void reconcileSessionStates();
      appendAppNote(res.session.id, `Opened ${escapeForTerminal(res.session.label)}.\r\n`);
    } catch (e: unknown) {
      setError(errorText(e));
    }
  }

  /** Asks first when a command is running; used by the tab X and both close chords. */
  async function requestCloseSession(sessionId: string) {
    if (sessionIsRunning(sessionId)) {
      const accepted = await askConfirm({
        title: "Close this session?",
        message: "A command is still running in this session. Close it and stop the command?",
        confirmLabel: "Close session",
      });
      if (!accepted) return;
    }
    void handleCloseSession(sessionId);
  }

  async function handleCloseSession(sessionId: string) {
    // Closing kills the shell without an ExecutionFinished for the command in flight, so
    // the rows still "planned" for it are closed out here (local store and database).
    const inFlightRows = sessionIsRunning(sessionId)
      ? useHistoryStore
          .getState()
          .items.filter(
            (h) => h.sessionId === sessionId && h.status === "planned" && h.executedCommand !== undefined,
          )
      : [];
    try {
      await closeSession({ sessionId });
      const finishedAt = new Date().toISOString();
      for (const row of inFlightRows) {
        updateHistoryItem(row.id, { status: "interrupted", finishedAt });
        persistInBackground(
          "history update",
          historyUpdate({ historyId: row.id, status: "interrupted", finishedAt }),
        );
      }
      forgetSession(sessionId);
      // Abort the workflow poll only once the session is really gone; aborting first
      // made the aborted branch report an idle session whose PTY was still busy.
      workflowAbortBySessionRef.current.get(sessionId)?.abort();
      removeSession(sessionId);
      delete terminalLinesBySessionRef.current[sessionId];
      clearSessionRunning(sessionId);
      inFlightExecRef.current.delete(sessionId);
      unlockSession(sessionId);
    } catch (e: unknown) {
      setError(errorText(e));
    }
  }

  // --- History action handlers ---
  async function handleRerunHistoryItem(item: HistoryItem) {
    if (!session) return;
    const command = item.executedCommand ?? item.generatedCommand;
    if (!command) return;
    if (item.status === "rejected") {
      // A rejected plan never ran; running it from here would skip the plan panel's risk gate.
      setError("That plan was rejected. Use View Plan to review it and run it through the risk check.");
      return;
    }
    if (sessionIsRunning(session.id) || busySessionsRef.current.has(session.id)) {
      setError(busyMessage(session.id));
      return;
    }

    if (sessionNotReady(session.id)) {
      setError(notReadyMessage(session.id));
      return;
    }
    const problem = commandProblem(command);
    if (problem) {
      setError(problem);
      return;
    }
    // A rerun skips the plan panel's risk checkbox, and the row does not record the risk it
    // was approved at: confirm when the directory differs or the command came from a plan.
    const cwdDiffers = item.cwd !== undefined && item.cwd !== session.cwd;
    if (cwdDiffers || item.source === "semantic") {
      const accepted = await askConfirm({
        title: "Run this again?",
        message:
          `Run again in "${session.label}" (${displayPath(session.cwd)})?\n\n${escapeForTerminal(command)}` +
          (cwdDiffers ? `\n\nIt first ran in ${displayPath(item.cwd)}.` : ""),
        confirmLabel: "Run again",
      });
      if (!accepted) return;
    }

    const sessionId = session.id;
    lockSession(sessionId);
    try {
      if (sessionIsRunning(sessionId)) {
        setError(busyMessage(sessionId));
        return;
      }
      inFlightExecRef.current.add(sessionId);
      const executionId = crypto.randomUUID();
      const historyItem: HistoryItem = {
        id: executionId,
        sessionId: session.id,
        source: item.source,
        userInput: item.userInput,
        executedCommand: command,
        linkedPlanId: item.linkedPlanId,
        status: "planned",
        createdAt: new Date().toISOString(),
        cwd: session.cwd,
        plannerSource: item.plannerSource,
      };
      appendHistoryItem(historyItem);
      executionToHistoryRef.current[executionId] = executionId;
      persistInBackground("history append", historyAppend({ item: historyItem }));

      try {
        await executeCommand({
          executionId,
          sessionId,
          command,
          source: item.source,
          linkedPlanId: item.linkedPlanId,
        });
      } catch (err) {
        failRejectedExecution(executionId, err);
        throw err;
      } finally {
        inFlightExecRef.current.delete(sessionId);
      }

      setHistoryOpen(false);
    } catch (e: unknown) {
      setError(errorText(e));
    } finally {
      unlockSession(sessionId);
    }
  }

  function handleReopenPlan(item: HistoryItem) {
    if (!item.generatedCommand) return;

    // History outlives sessions, so the row's session is often gone (earlier launch, closed
    // tab). Open the plan against the session the user is viewing and say so, instead of a
    // Run button that can only fail. With no open session it opens read-only.
    const live = sessions.find((s) => s.id === activeSessionIdRef.current) ?? null;
    const targetSessionId = live?.id ?? item.sessionId;
    if (!live) {
      setPlanNotice("No session is open, so this plan is read-only. Open a new session to run it.");
    } else if (item.sessionId !== live.id) {
      const original = sessions.some((s) => s.id === item.sessionId)
        ? "was made in another session"
        : "was made in a session that is closed";
      setPlanNotice(`This plan ${original}. It will run in "${live.label}" (${displayPath(live.cwd)}) instead.`);
    } else {
      setPlanNotice(null);
    }

    const syntheticPlan: CommandPlan = {
      id: item.linkedPlanId ?? crypto.randomUUID(),
      sessionId: targetSessionId,
      source: "semantic",
      userIntent: item.userInput,
      command: item.generatedCommand,
      explanation: "Reopened from history without a stored risk, so confirmation is required.",
      assumptions: [],
      confidence: 0.9,
      risk: "high",
      destructive: false,
      requiresConfirmation: true,
      touchesFiles: false,
      touchesNetwork: false,
      escalatesPrivileges: false,
      generatedAt: item.createdAt,
    };

    planGateRef.current = { command: "", confirmed: false };
    setPlan({
      plan: syntheticPlan,
      review: {
        planId: syntheticPlan.id,
        ambiguityFlags: [],
        safetyFlags: [],
        memoryUsed: [],
        retrievedContext: [],
      },
      status: {
        state: "ready",
        model: plannerModel,
        endpoint: plannerEndpoint,
        headline: "Ready.",
        fix: `Ask can draft a command with ${plannerModel}.`,
        link: "https://ollama.com/library",
        linkLabel: "Model library",
      },
    });
    setPlanNonce((n) => n + 1);
    // Do NOT adopt the historical id: reject/approve must not rewrite that row.
    setCurrentPlanHistoryId(null);
    setHistoryOpen(false);
  }

  async function handleSaveWorkflowFromHistory(item: HistoryItem) {
    if (!session) return;
    const command = item.executedCommand ?? item.generatedCommand;
    if (!command) return;

    const workflow: Workflow = {
      id: crypto.randomUUID(),
      label: item.userInput.slice(0, 48),
      source: item.source,
      originalIntent: item.source === "semantic" ? item.userInput : undefined,
      command,
      steps: [{ command }],
      projectRoot: session.cwd,
      createdAt: new Date().toISOString(),
    };

    try {
      await workflowAdd({ workflow });
    } catch (e: unknown) {
      setError(`Could not save the workflow: ${errorText(e)}`);
      return;
    }
    addWorkflow(workflow);
    appendAppNote(session.id, `Saved workflow ${escapeForTerminal(workflow.label)}.\r\n`);
    setHistoryOpen(false);
  }

  function handleNewWorkflow() {
    setWorkflowEditorData({
      workflowId: crypto.randomUUID(),
      label: "",
      steps: [""],
      projectRoot: session?.cwd,
      mode: "create",
    });
  }

  function handleEditWorkflow(workflow: Workflow) {
    const steps =
      workflow.steps && workflow.steps.length > 0
        ? workflow.steps.map((step) => step.command)
        : workflow.command
          ? [workflow.command]
          : [""];
    setWorkflowEditorData({
      workflowId: workflow.id,
      label: workflow.label,
      steps,
      projectRoot: workflow.projectRoot,
      createdAt: workflow.createdAt,
      source: workflow.source,
      originalIntent: workflow.originalIntent,
      mode: "edit",
    });
  }

  function handleSaveSelectedWorkflows(items: HistoryItem[]) {
    const chosen = items.filter((item) => item.executedCommand ?? item.generatedCommand);
    if (chosen.length === 0) return;
    const only = chosen.length === 1 ? chosen[0] : undefined;
    setWorkflowEditorData({
      workflowId: crypto.randomUUID(),
      label: only ? only.userInput.slice(0, 48) : "",
      steps: chosen.map((item) => item.executedCommand ?? item.generatedCommand ?? ""),
      projectRoot: session?.cwd,
      mode: "create",
    });
  }

  // --- Workflow run helpers ---
  function waitForStepCompletion(executionId: string, signal: AbortSignal) {
    return waitForTerminalStatus(
      () => useHistoryStore.getState().items.find((h) => h.id === executionId),
      (item) => item.status !== "planned",
      signal,
    );
  }

  function formatRunDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
    return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
  }

  function writeRunSummary(sessionId: string, run: WorkflowRun, finalStatus: "success" | "failed" | "interrupted") {
    const succeeded = run.steps.filter((s) => s.status === "success").length;
    const total = run.steps.length;
    const duration = run.finishedAt ? formatRunDuration(run.finishedAt - run.startedAt) : "";
    const durSuffix = duration ? ` (${duration})` : "";

    if (finalStatus === "success") {
      appendAppNote(sessionId, `Finished workflow ${escapeForTerminal(run.workflowName)}. ${succeeded} of ${total} steps worked${durSuffix}.\r\n`);
    } else if (finalStatus === "failed") {
      const failedStep = run.steps.find((s) => s.status === "failed");
      appendAppNote(sessionId, `Workflow ${escapeForTerminal(run.workflowName)} stopped. ${succeeded} of ${total} steps worked, and step ${(failedStep?.index ?? 0) + 1} did not${durSuffix}.\r\n`);
    } else {
      const skipped = run.steps.filter((s) => s.status === "skipped").length;
      const interruptedStep = run.steps.find((s) => s.status === "interrupted");
      appendAppNote(sessionId, `Workflow ${escapeForTerminal(run.workflowName)} was stopped during step ${(interruptedStep?.index ?? 0) + 1}. ${skipped} steps were skipped${durSuffix}.\r\n`);
    }
  }

  // --- Workflow run handler ---
  async function handleRunWorkflow(workflow: Workflow) {
    if (!session) return;
    const runSessionId = session.id;
    if (sessionIsRunning(runSessionId) || busySessionsRef.current.has(runSessionId)) {
      setError(busyMessage(runSessionId));
      return;
    }
    if (sessionNotReady(runSessionId)) {
      setError(notReadyMessage(runSessionId));
      return;
    }
    const stepCommands = workflow.steps?.map((s) => s.command) ?? [workflow.command];
    for (const stepCommand of stepCommands) {
      const problem = commandProblem(stepCommand);
      if (problem) {
        setError(`Workflow "${workflow.label}" was not started. ${problem}`);
        return;
      }
    }
    // A workflow records no risk; running it in a different directory than it was saved in is
    // the case worth a second look.
    if (workflow.projectRoot && workflow.projectRoot !== session.cwd) {
      const accepted = await askConfirm({
        title: "Run this workflow here?",
        message: `Run "${escapeForTerminal(workflow.label)}" in "${session.label}" (${displayPath(session.cwd)})? It was saved for ${displayPath(workflow.projectRoot)}.\n\n${stepCommands.map(escapeForTerminal).join("\n")}`,
        confirmLabel: "Run workflow",
      });
      if (!accepted) return;
    }
    lockSession(runSessionId);
    const controller = new AbortController();
    workflowAbortBySessionRef.current.set(runSessionId, controller);
    setWorkflowOpen(false);

    const runId = crypto.randomUUID();
    const commands = workflow.steps?.map((s) => s.command) ?? [workflow.command];
    const historySource: "raw" | "semantic" = workflow.source === "semantic" ? "semantic" : "raw";

    const runSteps: WorkflowStepRun[] = commands.map((cmd, i) => ({
      index: i,
      command: cmd,
      label: workflow.steps?.[i]?.label,
      status: "pending" as const,
    }));

    const run: WorkflowRun = {
      id: runId,
      workflowId: workflow.id,
      workflowName: workflow.label,
      startedAt: Date.now(),
      status: "running",
      currentStepIndex: 0,
      steps: runSteps,
    };
    setActiveRun(run);

    try {
      for (let i = 0; i < commands.length; i++) {
        const cmd = commands[i];
        const executionId = crypto.randomUUID();

        // Mark step running
        updateActiveRunStep(i, { status: "running", startedAt: Date.now() });

        // Create history item linked to this workflow run
        const stepLabel = commands.length > 1 ? ` [${i + 1}/${commands.length}]` : "";
        if (sessionIsRunning(runSessionId)) {
          updateActiveRunStep(i, { status: "failed", finishedAt: Date.now() });
          for (let j = i + 1; j < commands.length; j++) {
            updateActiveRunStep(j, { status: "skipped" });
          }
          const latestRun = useWorkflowRunStore.getState().activeRun;
          completeActiveRun("failed");
          if (latestRun) {
            writeRunSummary(runSessionId, { ...latestRun, finishedAt: Date.now() }, "failed");
          }
          setError(busyMessage(runSessionId));
          return;
        }

        const historyItem: HistoryItem = {
          id: executionId,
          sessionId: runSessionId,
          source: historySource,
          userInput: `${workflow.label}${stepLabel}`,
          executedCommand: cmd,
          status: "planned",
          createdAt: new Date().toISOString(),
          cwd: session.cwd,
          workflowRunId: runId,
        };
        appendHistoryItem(historyItem);
        executionToHistoryRef.current[executionId] = executionId;
        persistInBackground("history append", historyAppend({ item: historyItem }));

        inFlightExecRef.current.add(runSessionId);
        try {
          await executeCommand({
            executionId,
            sessionId: runSessionId,
            command: cmd,
            source: historySource,
          });
        } catch (err) {
          failRejectedExecution(executionId, err);
          throw err;
        } finally {
          inFlightExecRef.current.delete(runSessionId);
        }

        let waited = await waitForStepCompletion(executionId, controller.signal);
        // A long step (npm install, a build, a test suite) is not a failure. While the
        // terminal still reports the command running, keep waiting; only an explicit
        // Interrupt stops it.
        while (
          !waited.ok &&
          waited.reason === "timeout" &&
          execIsBusy(execStateOf(runSessionId))
        ) {
          appendAppNote(
            runSessionId,
            `Step ${i + 1} of ${commands.length} is still running. Choose Stop to end it.`,
          );
          waited = await waitForStepCompletion(executionId, controller.signal);
        }
        if (!waited.ok) {
          updateActiveRunStep(i, {
            status: "failed",
            finishedAt: Date.now(),
            historyItemId: executionId,
          });
          for (let j = i + 1; j < commands.length; j++) {
            updateActiveRunStep(j, { status: "skipped" });
          }
          const latestRun = useWorkflowRunStore.getState().activeRun;
          completeActiveRun("failed");
          if (latestRun) {
            writeRunSummary(runSessionId, { ...latestRun, finishedAt: Date.now() }, "failed");
          }
          // Reached only when the terminal is no longer running the step (a lost finish event)
          // or the run was aborted: nothing is interrupted here.
          clearSessionRunning(runSessionId);
          if (waited.reason === "timeout") {
            setError(
              "A workflow step never reported a result, but the terminal is idle. The run was stopped; check the terminal output.",
            );
          }
          return;
        }
        const finished = waited.item;
        const stepStatus = finished.status as "success" | "failure" | "interrupted";
        const mappedStatus = stepStatus === "failure" ? "failed" : stepStatus;

        updateActiveRunStep(i, {
          status: mappedStatus,
          finishedAt: Date.now(),
          historyItemId: executionId,
        });

        // Stop on failure or interruption
        if (mappedStatus !== "success") {
          // Mark remaining steps as skipped
          for (let j = i + 1; j < commands.length; j++) {
            updateActiveRunStep(j, { status: "skipped" });
          }
          // Read latest run state before completing
          const latestRun = useWorkflowRunStore.getState().activeRun;
          completeActiveRun(mappedStatus === "interrupted" ? "interrupted" : "failed");
          if (latestRun) {
            writeRunSummary(runSessionId, { ...latestRun, finishedAt: Date.now() }, mappedStatus === "interrupted" ? "interrupted" : "failed");
          }
          return;
        }
      }

      // All steps succeeded
      const latestRun = useWorkflowRunStore.getState().activeRun;
      completeActiveRun("success");
      if (latestRun) {
        writeRunSummary(runSessionId, { ...latestRun, finishedAt: Date.now() }, "success");
      }
    } catch (e: unknown) {
      // Mark remaining steps as skipped on unexpected error
      const latestRun = useWorkflowRunStore.getState().activeRun;
      if (latestRun) {
        for (const step of latestRun.steps) {
          if (step.status === "pending" || step.status === "running") {
            updateActiveRunStep(step.index, { status: "skipped" });
          }
        }
      }
      completeActiveRun("failed");
      setError(errorText(e));
    } finally {
      if (workflowAbortBySessionRef.current.get(runSessionId) === controller) {
        workflowAbortBySessionRef.current.delete(runSessionId);
      }
      unlockSession(runSessionId);
    }
  }

  // --- Workflow delete handler ---
  async function handleDeleteWorkflow(workflowId: string) {
    const workflow = useWorkflowStore.getState().items.find((item) => item.id === workflowId);
    if (!workflow) return;
    const accepted = await askConfirm({
      title: "Delete this workflow?",
      message: `Delete "${workflow.label}"? You can bring it back for 10 seconds.`,
      confirmLabel: "Delete workflow",
    });
    if (!accepted) return;
    removeWorkflow(workflowId);
    armUndo({
      token: `workflow:${workflowId}`,
      message: `Deleted ${workflow.label}.`,
      restore: () => addWorkflow(workflow),
      commit: () => {
        void workflowDelete({ id: workflowId }).catch((e: unknown) => {
          if (isNotFoundError(e)) return;
          addWorkflow(workflow);
          setError(errorText(e));
        });
      },
    });
  }

  // --- Cross-drawer navigation (Phase 6D) ---

  function handleViewWorkflowRun(workflowRunId: string) {
    // Find which workflow owns this run
    const entry = Object.entries(lastRunByWorkflowId).find(
      ([, run]) => run.id === workflowRunId,
    );
    if (!entry) return;
    const [workflowId] = entry;
    setOverlay("workflow");
    setExpandedRunWorkflowId(workflowId);
  }

  function handleRetryFailedStep(command: string) {
    setInputValue(command);
    setWorkflowOpen(false);
    setExpandedRunWorkflowId(null);
    requestAnimationFrame(() => composerRef.current?.focus());
  }

  function handleViewHistoryItemFromRun(historyItemId: string) {
    setExpandedRunWorkflowId(null);
    setHistoryInitialExpandedId(historyItemId);
    setOverlay("history");
  }

  // --- Memory handlers ---
  async function handleAcceptSuggestion(suggestionId: string) {
    // For workflow_pattern suggestions, open the editor instead of immediately creating
    // Read directly from store to avoid stale closure
    const suggestion = useMemoryStore.getState().suggestions.find((s) => s.id === suggestionId);
    if (suggestion?.kind === "workflow_pattern") {
      try {
        const parsed: unknown = JSON.parse(suggestion.proposedValue);
        if (!Array.isArray(parsed) || parsed.some((step) => typeof step !== "string")) {
          throw new Error("workflow pattern is not a list of commands");
        }
        const steps = parsed as string[];
        setWorkflowEditorData({
          workflowId: crypto.randomUUID(),
          suggestionId,
          label: suggestion.proposedKey,
          steps,
          projectRoot: suggestion.projectRoot,
          mode: "create",
        });
        return;
      } catch (error: unknown) {
        const detail = error instanceof Error ? error.message : "invalid workflow pattern";
        setError(`Could not read that workflow pattern (${detail}). Accepting it as a memory item instead.`);
      }
    }

    try {
      const res = await memoryAcceptSuggestion({ suggestionId });
      if (res.createdItem) {
        addMemoryItem(res.createdItem);
      }
      // Mark as accepted in store (don't remove — detectors need accepted state to avoid re-suggesting)
      setMemorySuggestions((prev) =>
        prev.map((s) =>
          s.id === suggestionId ? { ...s, status: "accepted" as const } : s,
        ),
      );
    } catch (e: unknown) {
      if (isNotFoundError(e)) {
        // The database no longer holds it as pending (accepted or dismissed earlier, or never
        // stored). Stop offering it, but say plainly that nothing was created: an explicit
        // Accept must not read as success or as a quiet dismissal.
        setMemorySuggestions((prev) =>
          prev.map((s) =>
            s.id === suggestionId ? { ...s, status: "dismissed" as const } : s,
          ),
        );
        setError(
          "That suggestion was already handled, so nothing new was saved and it was taken off the list.",
        );
        return;
      }
      setError(errorText(e));
    }
  }

  const confirmingRef = useRef(false);
  async function handleWorkflowEditorConfirm(label: string, steps: string[]) {
    if (!workflowEditorData || confirmingRef.current) return;
    confirmingRef.current = true;
    const data = workflowEditorData;
    const existing = useWorkflowStore.getState().items.find((item) => item.id === data.workflowId);

    try {
      // Persist first. The editor stays open until this succeeds so a failed
      // write does not throw away the name and steps.
      const workflow: Workflow = {
        id: data.workflowId,
        label,
        source: data.suggestionId ? "promoted" : (data.source ?? existing?.source ?? "raw"),
        originalIntent: data.originalIntent ?? existing?.originalIntent,
        command: steps.join(" && "),
        steps: steps.map((cmd) => ({ command: cmd })),
        projectRoot: data.projectRoot ?? existing?.projectRoot,
        createdAt: data.createdAt ?? existing?.createdAt ?? new Date().toISOString(),
      };
      await workflowAdd({ workflow });
      addWorkflow(workflow);
      setWorkflowEditorData(null);
      if (session) {
        const verb = data.mode === "edit" ? "Updated workflow" : "Saved workflow";
        appendAppNote(session.id, `${verb} ${escapeForTerminal(workflow.label)}.\r\n`);
      }
      if (!data.suggestionId) {
        setOverlay("workflow");
        return;
      }

      // Accept the memory suggestion. A failure here is partial success: the
      // workflow exists, so report it instead of discarding anything.
      const suggestionId = data.suggestionId;
      try {
        const res = await memoryAcceptSuggestion({ suggestionId });
        if (res.createdItem) {
          addMemoryItem(res.createdItem);
        }
        setMemorySuggestions((prev) =>
          prev.map((s) =>
            s.id === suggestionId ? { ...s, status: "accepted" as const } : s,
          ),
        );
      } catch (acceptErr: unknown) {
        if (isNotFoundError(acceptErr)) {
          setMemorySuggestions((prev) =>
            prev.map((s) =>
              s.id === suggestionId ? { ...s, status: "accepted" as const } : s,
            ),
          );
        } else {
          setError(
            `Workflow saved, but the suggestion could not be marked accepted: ${errorText(acceptErr)}`,
          );
        }
      }
    } catch (e: unknown) {
      setError(errorText(e));
    } finally {
      confirmingRef.current = false;
    }
  }

  function handleWorkflowEditorCancel() {
    setWorkflowEditorData(null);
  }

  async function handleDismissSuggestion(suggestionId: string) {
    try {
      await memoryDismissSuggestion({ suggestionId });
    } catch (e: unknown) {
      if (!isNotFoundError(e)) {
        setError(errorText(e));
        return;
      }
      // Not stored or already handled: dismissing locally is the intended end state.
    }
    // Mark as dismissed in store (don't remove — detectors need dismissed state to avoid re-suggesting)
    setMemorySuggestions((prev) =>
      prev.map((s) =>
        s.id === suggestionId ? { ...s, status: "dismissed" as const } : s,
      ),
    );
  }

  async function handleDeleteMemory(memoryId: string) {
    const item = useMemoryStore.getState().items.find((entry) => entry.id === memoryId);
    if (!item) return;
    const accepted = await askConfirm({
      title: "Delete this memory?",
      message: `Delete "${item.key}"? You can bring it back for 10 seconds.`,
      confirmLabel: "Delete memory",
    });
    if (!accepted) return;
    removeMemoryItem(memoryId);
    armUndo({
      token: `memory:${memoryId}`,
      message: `Deleted ${item.key}.`,
      restore: () => addMemoryItem(item),
      commit: () => {
        void memoryDelete({ memoryId }).catch((e: unknown) => {
          if (isNotFoundError(e)) return;
          addMemoryItem(item);
          setError(errorText(e));
        });
      },
    });
  }

  // --- Suggestion generation ---
  async function generateSuggestions() {
    const history = useHistoryStore.getState().items;
    const { items: mem, suggestions: sug } = useMemoryStore.getState();
    const { sessions: liveSessions, activeSessionId: liveActiveId } = useSessionStore.getState();
    const session = liveSessions.find((s) => s.id === liveActiveId);

    const candidates = runDetectors({
      history,
      existingSuggestions: sug,
      existingMemory: mem,
      projectRoot: session?.cwd,
    });

    // Only suggestions that reached the database go into state; a suggestion the
    // backend does not know could never be accepted or dismissed.
    const stored: MemorySuggestion[] = [];
    for (const candidate of candidates) {
      if (settledSuggestionIdsRef.current.has(candidate.id)) continue;
      try {
        const res = (await memoryStoreSuggestion({ suggestion: candidate })) as {
          ok?: boolean;
          inserted?: boolean;
        };
        // INSERT OR IGNORE reports success for an id the database already settled; when the
        // backend says nothing was inserted, the row is accepted or dismissed, not pending.
        if (res?.inserted === false) {
          settledSuggestionIdsRef.current.add(candidate.id);
          continue;
        }
        stored.push(candidate);
      } catch (e: unknown) {
        console.warn("[memory] suggestion store failed:", e instanceof Error ? e.message : e);
      }
    }

    if (stored.length > 0) {
      // Use function-form setter so the store's built-in dedup runs on latest state
      setMemorySuggestions((prev) => [...prev, ...stored]);
    }
  }

  // --- Plan display ---
  const planTarget = plan ? sessions.find((s) => s.id === plan.plan.sessionId) ?? null : null;
  let planBlockedReason: string | undefined;
  if (plan) {
    if (!planTarget) {
      planBlockedReason = activeSessionId
        ? "The session this plan was made for is closed."
        : "No session is open. Open a new session to run this plan.";
    } else if (exitedSessions.has(planTarget.id)) {
      planBlockedReason = `The shell in "${planTarget.label}" has exited.`;
    } else if (planTarget.id !== activeSessionId) {
      planBlockedReason = `This plan runs in "${planTarget.label}", but you are viewing another session.`;
    }
  }

  const offerText = activeSessionId ? requestOffers[activeSessionId] : undefined;
  const sessionResult = activeSessionId ? resultsBySession[activeSessionId] : undefined;
  const shownResult = offerText
    ? describeResult({ phase: "request", command: offerText })
    : isRunning
      ? describeResult({ phase: "running", command: sessionResult?.command })
      : sessionResult?.view ?? null;

  function onResultAction(action: ResultAction) {
    if (!activeSessionId) return;
    if (action === "stop") {
      void handleInterrupt();
      return;
    }
    if (action === "show-output") {
      const sessionId = activeSessionId;
      setResultsBySession((prev) => {
        const current = prev[sessionId];
        if (!current) return prev;
        return { ...prev, [sessionId]: { ...current, outputOpen: !current.outputOpen } };
      });
      return;
    }
    if (action === "ask-instead") {
      const text = requestOffers[activeSessionId];
      if (!text) return;
      const sessionId = activeSessionId;
      setRequestOffers((prev) => {
        if (!(sessionId in prev)) return prev;
        const next = { ...prev };
        delete next[sessionId];
        return next;
      });
      setInputMode("ask");
      composerRef.current?.setValue(text);
      composerRef.current?.focus();
      return;
    }
    const current = resultsBySession[activeSessionId];
    if (!current) return;
    if (action === "ask-fix") {
      const prompt = askFixPrompt({
        command: current.command,
        exitCode: current.exitCode,
        exitKnown: current.exitKnown,
        outputText: current.output,
        cause: current.view.cause,
      });
      setInputMode("ask");
      composerRef.current?.setValue(prompt);
      void handleSubmit(prompt, "ask").then((ok) => {
        if (ok) composerRef.current?.setValue("");
      });
      return;
    }
    if (action === "run-again" && current.command) {
      const command = current.command;
      composerRef.current?.setValue(command);
      void handleSubmit(command, "command").then((ok) => {
        if (ok) composerRef.current?.setValue("");
      });
    }
  }

  async function refreshPlannerStatus() {
    const session = sessions.find((item) => item.id === activeSessionId) ?? sessions[0];
    try {
      const res = await generatePlan({
        sessionId: session?.id ?? "none",
        userIntent: "",
        probeOnly: true,
        model: plannerModel,
        endpoint: plannerEndpoint,
        context: buildPlannerContext({
          sessionId: session?.id ?? "none",
          cwd: session?.cwd ?? ".",
          shell: session?.shell ?? "unknown",
          os: detectOS(),
          memoryItems,
          workflows,
          lastRunByWorkflowId,
          recentHistory: historyItems,
        }),
      });
      setPlannerStatus(res.status);
    } catch {
      setPlannerStatus({
        state: "unavailable",
        model: plannerModel,
        endpoint: plannerEndpoint,
        headline: "CommandUI could not check the model.",
        fix: "Choose Check again in a moment.",
        link: "https://ollama.com/download",
        linkLabel: "Download Ollama",
      });
    }
  }

  const showPlanColumn =
    productMode === "guided" || plan !== null || (plannerStatus !== null && plannerStatus.state !== "ready");

  const displayExplanation =
    plan && simplifiedSummaries
      ? simplifyText(plan.plan.explanation)
      : plan?.plan.explanation ?? "";

  const planIntent = plan?.plan.userIntent ?? "";
  const hasPlan = plan !== null;
  const finishedText = shownResult && shownResult.announce ? resultText(shownResult) : "";
  const resultToken = finishedText
    ? `${activeSessionId ?? ""}:${finishedText}:${sessionResult?.exitKnown ?? ""}:${sessionResult?.exitCode ?? ""}`
    : "";

  useEffect(() => {
    if (!hasPlan) return;
    const text = `A plan is ready to review. ${planIntent}`;
    const timer = window.setTimeout(() => {
      setLiveMessage((prev) => (prev === text ? `${text}\u200b` : text));
    }, 150);
    document.querySelector<HTMLElement>(".plan-panel")?.focus();
    return () => window.clearTimeout(timer);
  }, [hasPlan, planNonce, planIntent]);

  useEffect(() => {
    if (!finishedText) return;
    const text = finishedText;
    const timer = window.setTimeout(() => {
      setLiveMessage((prev) => (prev === text ? `${text}\u200b` : text));
    }, 150);
    return () => window.clearTimeout(timer);
  }, [resultToken, finishedText]);

  useEffect(() => {
    if (!settingsOpen) return;
    void refreshPlannerStatus();
    // Opening Settings is the check. Editing the model uses Check again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsOpen]);

  const shellStyle = { "--ui-scale": String(fontScale(fontSize)) } as CSSProperties;

  // --- Boot failure screen ---
  if (bootPhase === "failed") {
    return (
      <div className="app-shell" style={shellStyle}>
        <header className="topbar">
          <div>
            <strong>CommandUI</strong>
          </div>
          <div className="topbar-actions">
            <button type="button" onClick={() => toggleOverlay("help")}>
              Keyboard help
            </button>
          </div>
        </header>
        <div className="boot-failure">
          <h2>CommandUI could not start</h2>
          <p className="muted">{bootError}</p>
          <div className="boot-failure-actions">
            <button type="button" onClick={() => window.location.reload()}>
              Retry
            </button>
            <button
              type="button"
              onClick={() => {
                void navigator.clipboard.writeText(bootError ?? "Unknown error");
              }}
            >
              Copy Error
            </button>
          </div>
          <p className="muted boot-failure-hint">
            If this keeps happening, choose Retry. If CommandUI still does not start, choose Copy Error and include that text when you ask for help.
          </p>
        </div>
        {helpOpen && <HelpDialog onClose={() => setHelpOpen(false)} />}
      </div>
    );
  }

  // --- Render ---
  return (
    <div className="app-shell" style={shellStyle}>
      <header className="topbar">
        <h1 className="visually-hidden">CommandUI</h1>
        <div className="visually-hidden" role="status" aria-atomic="true">
          {liveMessage}
        </div>
        <div>
          <strong>CommandUI</strong>
          <span className="muted"> v{APP_VERSION}</span>
          {session && (
            <span className="muted" title={session.cwd ?? undefined}>
              {" "}— {session.cwd ? displayPath(session.cwd) : session.label}
            </span>
          )}
        </div>
        <div className="topbar-actions">
          <button type="button" onClick={() => setOverlay("output")}>
            Output
          </button>
          <button type="button" onClick={() => setOverlay("history")}>
            History
          </button>
          <button type="button" onClick={() => setOverlay("workflow")}>
            Workflows
          </button>
          <button type="button" onClick={() => setOverlay("memory")}>
            Memory
          </button>
          <button type="button" onClick={() => setOverlay("settings")}>
            Settings
          </button>
          <button type="button" onClick={() => toggleOverlay("help")}>
            Keyboard help
          </button>
        </div>
      </header>

      {undoMessage && (
        <div className="undo-bar" role="status">
          <span>{undoMessage}</span>
          <button type="button" onClick={undoPending}>
            Undo
          </button>
        </div>
      )}

      {browserPreview && (
        <div className="preview-banner">
          You are looking at CommandUI in a browser. Commands do not run here. Open the CommandUI application to use your terminal.
        </div>
      )}

      <main
        className={`main-layout ${!showPlanColumn ? "classic-no-plan" : ""}`}
      >
        <section className="terminal-column">
          <SessionTabs
            sessions={sessions}
            activeSessionId={activeSessionId}
            onSelect={setActiveSessionId}
            onCreate={handleCreateSession}
            onClose={requestCloseSession}
            exitedSessionIds={exitedSessions}
          />

          {activeExited && (
            <div className="exit-banner" role="status">
              <span>Shell exited. Open a new session to continue.</span>
              <button type="button" onClick={handleCreateSession}>
                New Session
              </button>
            </div>
          )}

          {activeExecState === "desynced" && !activeExited && (
            <div className="session-banner">
              <span>The terminal lost track of this session. Choose Resync to connect it again.</span>
              <button type="button" onClick={handleResync}>
                Resync
              </button>
            </div>
          )}

          {activeBootStalled && !activeExited && activeSessionId && (
            <div className="session-banner" role="status">
              <span>
                This session has not finished starting. A startup script may have stopped the shell.
                Choose Resync, or close the session and open a new one.
              </span>
              <button type="button" onClick={handleResync}>
                Resync
              </button>
              <button type="button" onClick={() => requestCloseSession(activeSessionId)}>
                Close Session
              </button>
            </div>
          )}

          {activeForegroundStuck && !activeExited && activeSessionId && (
            <div className="session-banner" role="status">
              <span>
                {activeExecState === "interrupting"
                  ? "Stop has not ended the command."
                  : "A command you typed has held this session for a while."}{" "}
                A program you started, such as another shell or a remote login, stays open until you type exit in the terminal.
                Resync is not offered here because it would type into that program.
              </span>
              <button type="button" onClick={() => requestCloseSession(activeSessionId)}>
                Close Session
              </button>
            </div>
          )}

          <WorkflowRunBanner />

          {bootPhase === "booting" && (
            <div className="boot-loading">Starting CommandUI...</div>
          )}

          <ResultLine
            result={shownResult}
            outputOpen={sessionResult?.outputOpen ?? false}
            outputText={sessionResult?.output ?? ""}
            onAction={onResultAction}
          />

          <TerminalPane
            ref={terminalPaneRef}
            sessionId={activeSessionId}
            executionStatus={visibleExecutionStatus}
            onResize={handleTerminalResize}
            onData={handleTerminalData}
            autoFocus
          />

          {activeNotes.length > 0 && (
            <div className="app-notes" role="log" aria-label="CommandUI activity">
              {activeNotes.slice(-APP_NOTES_SHOWN).map((note, i) => (
                <div key={activeNotes.length - APP_NOTES_SHOWN + i} className="app-note">
                  {note}
                </div>
              ))}
              <button type="button" className="link-btn" onClick={clearTerminalView}>
                Clear
              </button>
            </div>
          )}

          {error && (
            <div className="error-box" role="alert">
              <span>{error}</span>
              <button type="button" onClick={() => setError(null)}>
                Dismiss
              </button>
            </div>
          )}

          <MemorySuggestions
            suggestions={memorySuggestions.filter(
              (s) => s.status === "pending",
            )}
            onAccept={handleAcceptSuggestion}
            onDismiss={handleDismissSuggestion}
          />

          <InputComposer
            ref={composerRef}
            mode={inputMode}
            onModeChange={setInputMode}
            onSubmit={handleSubmit}
            busy={composerBusy}
            isRunning={isRunning}
            onInterrupt={handleInterrupt}
            disabled={composerDisabled(activeExecState, activeExited)}
            disabledReason={
              activeExited
                ? "The shell exited. Open a new session to continue."
                : activeExecState === "desynced"
                  ? "The terminal lost track of this session. Choose Resync."
                  : activeBootStalled
                    ? "The terminal has not finished starting. Choose Resync, or close the session."
                    : activeExecState === "booting"
                      ? "The terminal is still starting."
                      : activeExecState === "userRunning"
                        ? "A command you typed is running. Wait for the prompt."
                        : undefined
            }
          />
        </section>

        {showPlanColumn && (
          <aside className="plan-column">
            {plan ? (
              <PlanPanel
                key={planNonce}
                sessionId={plan.plan.sessionId}
                intent={plan.plan.userIntent}
                command={plan.plan.command}
                risk={plan.plan.risk}
                explanation={displayExplanation}
                contextSources={plan.review.retrievedContext}
                plannerSource={plan.plan.source}
                flags={plan.plan}
                safetyFlags={plan.review.safetyFlags}
                ambiguityFlags={plan.review.ambiguityFlags}
                target={
                  planTarget ? { label: planTarget.label, cwd: planTarget.cwd } : undefined
                }
                blockedReason={planBlockedReason}
                notice={planNotice ?? undefined}
                onGoToTarget={
                  planTarget && !exitedSessions.has(planTarget.id) && planTarget.id !== activeSessionId
                    ? handleGoToPlanSession
                    : undefined
                }
                onRetarget={
                  planBlockedReason && activeSessionId && !activeExited && planTarget?.id !== activeSessionId
                    ? handleRetargetPlan
                    : undefined
                }
                onRunGate={acceptPlanRunGate}
                onApprove={handleApprovePlan}
                onReject={handleRejectPlan}
                onSaveWorkflow={handleSaveWorkflow}
              />
            ) : plannerStatus && plannerStatus.state !== "ready" ? (
              <div className="plan-panel" tabIndex={0} aria-label="Model status">
                <PlannerStatusCard
                  status={plannerStatus}
                  onCheckAgain={() => void refreshPlannerStatus()}
                />
              </div>
            ) : (
              <div className="plan-panel">
                <p className="muted">
                  Switch to <strong>Ask</strong> and describe a task. Its plan shows up here
                  for you to review before anything runs.
                </p>
              </div>
            )}
          </aside>
        )}
      </main>

      {welcomeOpen && (
        <WelcomeScreen
          onStart={closeWelcome}
          showAtStartup={showWelcomeAtStartup}
          onShowAtStartupChange={changeShowWelcome}
          plannerModel={plannerModel}
        />
      )}

      <HistoryDrawer
        isOpen={historyOpen}
        items={visibleHistoryItems}
        allItems={historyItems}
        sessions={sessions}
        activeSessionId={activeSessionId}
        onClose={() => {
          setHistoryOpen(false);
          setHistoryInitialExpandedId(null);
        }}
        onRerun={handleRerunHistoryItem}
        onReopenPlan={handleReopenPlan}
        onSaveWorkflow={handleSaveWorkflowFromHistory}
        onSaveSelected={handleSaveSelectedWorkflows}
        onCopyCommand={(cmd) => { void navigator.clipboard.writeText(cmd); }}
        onViewWorkflowRun={handleViewWorkflowRun}
        initialExpandedId={historyInitialExpandedId}
        loading={bootPhase === "booting"}
      />

      <WorkflowDrawer
        isOpen={workflowOpen}
        workflows={workflows}
        lastRunByWorkflowId={lastRunByWorkflowId}
        expandedRunWorkflowId={expandedRunWorkflowId}
        onClose={() => {
          setWorkflowOpen(false);
          setExpandedRunWorkflowId(null);
        }}
        onRun={handleRunWorkflow}
        onDelete={handleDeleteWorkflow}
        onNew={handleNewWorkflow}
        onEdit={handleEditWorkflow}
        onExpandRun={setExpandedRunWorkflowId}
        onRetryStep={handleRetryFailedStep}
        onCopyCommand={(cmd) => void navigator.clipboard.writeText(cmd)}
        onViewHistoryItem={handleViewHistoryItemFromRun}
        loading={bootPhase === "booting"}
      />

      <MemoryDrawer
        isOpen={memoryOpen}
        items={memoryItems}
        onClose={() => {
          setMemoryOpen(false);
        }}
        onDelete={handleDeleteMemory}
        loading={bootPhase === "booting"}
      />

      <CommandPalette
        isOpen={paletteOpen}
        onClose={() => {
          setPaletteOpen(false);
        }}
        actions={paletteActions}
      />

      <SettingsDrawer
        isOpen={settingsOpen}
        onClose={() => {
          setSettingsOpen(false);
        }}
        productMode={productMode}
        onProductModeChange={setProductMode}
        defaultInputMode={defaultInputMode}
        onDefaultInputModeChange={setDefaultInputMode}
        fontSize={fontSize}
        onFontSizeChange={setFontSize}
        simplifiedSummaries={simplifiedSummaries}
        onSimplifiedSummariesChange={setSimplifiedSummaries}
        plannerModel={plannerModel}
        onPlannerModelChange={setPlannerModel}
        plannerEndpoint={plannerEndpoint}
        onPlannerEndpointChange={setPlannerEndpoint}
        plannerStatus={plannerStatus}
        onCheckPlanner={() => void refreshPlannerStatus()}
      />

      {helpOpen && <HelpDialog onClose={() => setHelpOpen(false)} />}
      {outputOpen && (
        <OutputView
          blocks={activeSessionId ? (outputBlocksBySession[activeSessionId] ?? []) : []}
          onClose={() => setOutputOpen(false)}
        />
      )}

      {workflowEditorData && (
        <WorkflowEditor
          initialLabel={workflowEditorData.label}
          initialSteps={workflowEditorData.steps}
          projectRoot={workflowEditorData.projectRoot}
          mode={workflowEditorData.mode}
          onConfirm={handleWorkflowEditorConfirm}
          onCancel={handleWorkflowEditorCancel}
        />
      )}

      {pendingConfirm && (
        <ConfirmDialog
          title={pendingConfirm.title}
          message={pendingConfirm.message}
          confirmLabel={pendingConfirm.confirmLabel}
          onConfirm={pendingConfirm.onConfirm}
          onCancel={cancelConfirm}
        />
      )}
    </div>
  );
}
