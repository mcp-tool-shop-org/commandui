import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import { commandProblem, escapeForTerminal } from "../lib/displaySafe";
import type { PlanRunGate } from "../components/PlanPanel";
import { TerminalPane } from "../components/TerminalPane";
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
import { WorkflowRunBanner } from "../components/WorkflowRunBanner";
import { isTauriRuntime } from "../lib/tauriInvoke";
import { onMockEvent } from "../lib/mockBridge";
import { waitForTerminalStatus } from "./workflowStepWait";
import {
  busyMessageFor,
  composerDisabled,
  execIsBusy,
  execIsForeground,
  execOwnsBadge,
} from "./execState";
import {
  APP_NOTES_SHOWN,
  APP_VERSION,
  EXEC_STATES,
  SESSION_EXITED_MESSAGE,
  SESSION_NOT_READY_MESSAGE,
  SESSION_WATCH_MS,
  bootStallDue,
  capNotes,
  detectOS,
  foregroundStuckDue,
  simplifyText,
} from "./sessionWatch";
import { appendReplayChunk, privateModePrefix, type ReplaySession } from "./terminalReplay";

function execStateOf(sessionId: string): SessionExecState | undefined {
  return useExecutionStore.getState().sessionExecStates[sessionId];
}

function busyMessage(sessionId: string): string {
  return busyMessageFor(execStateOf(sessionId));
}

type SessionSummaryWithState = SessionSummary & { execState?: string | null };

type SessionBadgeStatus = "idle" | "running" | "success" | "failure";

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
    reducedClutter,
    simplifiedSummaries,
    confirmMediumRisk,
    defaultInputMode,
    setProductMode,
    setReducedClutter,
    setSimplifiedSummaries,
    setConfirmMediumRisk,
    setDefaultInputMode,
  } = useSettingsStore();
  const { items: workflows, setWorkflows, addWorkflow, removeWorkflow } = useWorkflowStore();
  const { setActiveRun, updateActiveRunStep, completeActiveRun } = useWorkflowRunStore();
  const lastRunByWorkflowId = useWorkflowRunStore((s) => s.lastRunByWorkflowId);
  const { restorePreviousZone } = useFocusStore();

  // --- Local state ---
  const [plan, setPlan] = useState<PlannerGeneratePlanResponse | null>(null);
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
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [workflowEditorData, setWorkflowEditorData] = useState<{
    workflowId: string;
    suggestionId: string;
    label: string;
    steps: string[];
    projectRoot?: string;
  } | null>(null);

  // Background buffer for session-switch replay
  // A ref (not state): appending must not re-render per PTY chunk. Capped per
  // session; xterm keeps the real scrollback.
  const terminalLinesBySessionRef = useRef<Record<string, string[]>>({});
  const executionToHistoryRef = useRef<Record<string, string>>({});
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
      return { ...prev, [sessionId]: capNotes(list) };
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
    // Every PTY chunk is kept and shown: a read of real output can begin with any text, so
    // reduced clutter must never drop one. (It applies to the app's own annotations.)
    const buffers = terminalLinesBySessionRef.current;
    const current: ReplaySession = {
      buffer: buffers[sessionId] ?? [],
      chars: replayCharsRef.current[sessionId] ?? 0,
      altScreen: altScreenRef.current[sessionId] ?? false,
      altTail: altTailRef.current[sessionId] ?? "",
      privateModes: privateModesRef.current[sessionId] ?? {},
      privateModeTail: privateModeTailRef.current[sessionId] ?? "",
    };
    const next = appendReplayChunk(current, line);
    buffers[sessionId] = next.buffer;
    replayCharsRef.current[sessionId] = next.chars;
    altScreenRef.current[sessionId] = next.altScreen;
    altTailRef.current[sessionId] = next.altTail;
    privateModesRef.current[sessionId] = next.privateModes;
    privateModeTailRef.current[sessionId] = next.privateModeTail;

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
            if (typeof s.reducedClutter === "boolean") setReducedClutter(s.reducedClutter);
            if (typeof s.simplifiedSummaries === "boolean") setSimplifiedSummaries(s.simplifiedSummaries);
            if (typeof s.confirmMediumRisk === "boolean") setConfirmMediumRisk(s.confirmMediumRisk);
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
        const msg = e instanceof Error ? e.message : String(e);
        setBootError(msg);
        setBootPhase("failed");
        setError(`Boot failed: ${msg}`);
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
    publishBadge(sessionId, "running");
    noteExecState(sessionId, "running");
    if (sessionId === activeSessionIdRef.current) {
      setActiveExecution(event.execution.id);
    }
  }

  function noteExecutionFinished(event: TerminalExecutionFinishedEvent) {
    const status = event.status;
    const badge: SessionBadgeStatus =
      status === "failure" ? "failure" : status === "success" ? "success" : "idle";
    if (event.sessionId) {
      publishBadge(event.sessionId, badge);
      const exec = useExecutionStore.getState().sessionExecStates[event.sessionId];
      if (exec === "running" || exec === "interrupting") {
        setSessionExecState(event.sessionId, "ready");
      }
      if (event.sessionId === activeSessionIdRef.current) {
        setActiveExecution(null);
      }
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
      exitCode: event.exitCode,
      finishedAt,
      durationMs,
    });
    persistInBackground("history update", historyUpdate({
      historyId,
      status,
      exitCode: event.exitCode,
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
        const msg = e instanceof Error ? e.message : String(e);
        console.error("[AppShell] Listener setup failed:", msg);
        setError(`A background initialization step failed: ${msg}. Some features may not work correctly.`);
      });

    return () => {
      disposed = true;
      for (const subscription of pending) {
        void subscription
          .then((unlisten) => {
            unlisten();
          })
          .catch((e: unknown) => {
            const msg = e instanceof Error ? e.message : String(e);
            console.warn("[AppShell] Unsubscribe failed:", msg);
          });
      }
    };
    // Subscriptions live for the whole shell: appendTerminalLine reads reducedClutter from
    // getState(), and resubscribing is asynchronous so events in the gap would be lost.
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
        if (state === "booting" && bootStallDue(now - at, stalledBootRef.current.has(sid))) {
          void reconcileSessionStates()
            .then(() => {
              if ((execStateOf(sid) ?? "booting") !== "booting") return;
              setStalledBoot((prev) => (prev.has(sid) ? prev : new Set(prev).add(sid)));
            })
            .catch((e: unknown) => {
              const msg = e instanceof Error ? e.message : String(e);
              console.error("[AppShell] Reconcile session states failed during stall check:", msg);
              setError(`Session sync failed: ${msg}. Click Resync if a session appears stuck.`);
            });
        } else if (
          execIsForeground(state) &&
          foregroundStuckDue(now - at, stuckForegroundRef.current.has(sid))
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
        reducedClutter,
        simplifiedSummaries,
        confirmMediumRisk,
        defaultInputMode,
      },
    }));
  }, [browserPreview, productMode, reducedClutter, simplifiedSummaries, confirmMediumRisk, defaultInputMode]);

  // --- Background persistence failure banner ---
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as {
        what: string;
        message: string;
      };
      setError(`Background save failed (${detail.what}): ${detail.message}. Your data may not be persisted.`);
    };
    window.addEventListener("commandui:persist-failed", handler);
    return () => window.removeEventListener("commandui:persist-failed", handler);
  }, []);

  // --- Centralized keyboard shortcuts ---
  const overlayRef = useRef({
    historyOpen: false,
    workflowOpen: false,
    memoryOpen: false,
    settingsOpen: false,
    paletteOpen: false,
    editorOpen: false,
    planOpen: false,
  });
  overlayRef.current = {
    historyOpen,
    workflowOpen,
    memoryOpen,
    settingsOpen,
    paletteOpen,
    editorOpen: workflowEditorData !== null,
    planOpen: plan !== null,
  };

  function closeAllOverlays() {
    const overlay = overlayRef.current;
    if (overlay.editorOpen) { setWorkflowEditorData(null); return; }
    if (overlay.paletteOpen) { setPaletteOpen(false); return; }
    if (overlay.historyOpen || overlay.workflowOpen || overlay.memoryOpen || overlay.settingsOpen) {
      setHistoryOpen(false);
      setWorkflowOpen(false);
      setMemoryOpen(false);
      setSettingsOpen(false);
      requestAnimationFrame(() => {
        restorePreviousZone();
        composerRef.current?.focus();
      });
      return;
    }
    // Escape at the shell prompt belongs to the shell (readline, completion): it must not
    // also reject the pending plan. Reject stays on the plan's R key and button.
    if (overlay.planOpen && useFocusStore.getState().currentZone !== "terminal") {
      handleRejectPlan();
    }
  }

  function focusComposer() {
    composerRef.current?.focus();
  }

  const shortcuts = useMemo<ShortcutDef[]>(() => {
    const defs: ShortcutDef[] = [
      { id: "palette",       combo: "ctrl+k",       context: ["global"], action: () => setPaletteOpen(true) },
      // Ctrl+Shift variants: the only app chords that work while the terminal has focus
      // (plain Ctrl+<letter> goes to the shell there).
      { id: "palette-term",  combo: "ctrl+shift+k", context: ["global"], action: () => setPaletteOpen(true) },
      { id: "focus-composer-term", combo: "ctrl+shift+j", context: ["global"], action: focusComposer },
      { id: "clear-terminal-term", combo: "ctrl+shift+l", context: ["global"], action: clearTerminalView },
      { id: "new-session-term", combo: "ctrl+shift+t", context: ["global"], action: handleCreateSession },
      { id: "history-term",   combo: "ctrl+shift+h", context: ["global"], action: () => setHistoryOpen((v) => !v) },
      { id: "memory-term",    combo: "ctrl+shift+m", context: ["global"], action: () => setMemoryOpen((v) => !v) },
      { id: "focus-composer", combo: "ctrl+j",       context: ["global"], action: focusComposer },
      { id: "clear-terminal", combo: "ctrl+l",       context: ["global"], action: clearTerminalView },
      { id: "new-session",   combo: "ctrl+t",        context: ["global"], action: handleCreateSession },
      { id: "history",       combo: "ctrl+h",        context: ["global"], action: () => setHistoryOpen((v) => !v) },
      { id: "workflows",     combo: "ctrl+shift+w",  context: ["global"], action: () => setWorkflowOpen((v) => !v) },
      { id: "memory",        combo: "ctrl+m",        context: ["global"], action: () => setMemoryOpen((v) => !v) },
      { id: "settings",      combo: "ctrl+,",        context: ["global"], action: () => setSettingsOpen((v) => !v) },
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
      { id: "open-history",    label: "Open History",      shortcut: "Ctrl+H",       action: () => setHistoryOpen(true) },
      { id: "open-workflows",  label: "Open Workflows",   shortcut: "Ctrl+Shift+W", action: () => setWorkflowOpen(true) },
      { id: "open-memory",     label: "Open Memory",      shortcut: "Ctrl+M",       action: () => setMemoryOpen(true) },
      { id: "open-settings",   label: "Open Settings",    shortcut: "Ctrl+,",       action: () => setSettingsOpen(true) },
      ...sessions.map((s, i) => ({
        id: `switch-session-${s.id}`,
        label: `Switch to ${s.label ?? `Session ${i + 1}`}`,
        shortcut: i < 9 ? `Ctrl+${i + 1}` : undefined,
        action: () => setActiveSessionId(s.id),
      })),
    ];

    if (isRunning) {
      actions.push({ id: "interrupt", label: "Interrupt Command", action: handleInterrupt });
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
  async function handleSubmit(value: string): Promise<boolean> {
    if (!session) return false;
    if (busySessionsRef.current.has(session.id)) return false;
    if (inputMode === "command" && sessionIsRunning(session.id)) {
      setError(busyMessage(session.id));
      return false;
    }
    const sessionId = session.id;
    if (inputMode === "command" && sessionNotReady(sessionId)) {
      setError(notReadyMessage(sessionId));
      return false;
    }
    if (inputMode === "command") {
      // Checked before any history row exists: a command the backend would refuse
      // (multi-line, control character) or that hides text must not leave a failure row.
      const problem = commandProblem(value);
      if (problem) {
        setError(problem);
        return false;
      }
    }
    let accepted = true;
    lockSession(sessionId);
    setError(null);

    try {
      if (inputMode === "command") {
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
        });

        // The user switched tabs while the plan was generating: drop it rather than show
        // a plan for another session that Approve could run in the wrong place.
        if (activeSessionIdRef.current !== sessionId) return true;

        planGateRef.current = { command: "", confirmed: false };
        setPlanNotice(null);
        setPlan(res);
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
          plannerSource: browserPreview ? "mock" : "ollama",
        };
        appendHistoryItem(historyItem);

        persistInBackground("history append", historyAppend({ item: historyItem }));
        persistInBackground("plan store", planStore({ plan: res.plan }));

        // Lines the app writes itself are escaped: model text must not move the cursor
        // or hide characters in the transcript.
        appendAppNote(session.id, `? ${escapeForTerminal(value)}\r\n`);
        appendAppNote(session.id, `[plan] ${escapeForTerminal(res.plan.command)}\r\n`);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
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
        requireMediumRiskConfirmation: confirmMediumRisk,
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

      appendAppNote(session.id, `[approved] ${escapeForTerminal(trimmed)}\r\n`);

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
            `Could not save the memory suggestion: ${storeErr instanceof Error ? storeErr.message : String(storeErr)}`,
          );
        }
      }

      setPlan(null);
      setPlanNotice(null);
      setCurrentPlanHistoryId(null);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
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
    if (noteSessionId) appendAppNote(noteSessionId, "[rejected]\r\n");
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
      projectRoot: session.cwd,
      createdAt: new Date().toISOString(),
    };

    // Show the workflow only once it is stored, or it would be undeletable.
    try {
      await workflowAdd({ workflow });
    } catch (e: unknown) {
      setError(`Could not save the workflow: ${e instanceof Error ? e.message : String(e)}`);
      return;
    }
    addWorkflow(workflow);
    appendAppNote(session.id, `[workflow:saved] ${escapeForTerminal(workflow.label)}\r\n`);
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
        const msg = e instanceof Error ? e.message : String(e);
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
      const msg = e instanceof Error ? e.message : String(e);
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
      const msg = e instanceof Error ? e.message : String(e);
      if (/has exited/i.test(msg)) {
        // Not a desync: the shell is gone. Show it as exited instead of an error.
        markSessionExited(activeSessionId);
        return;
      }
      setError(msg);
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
      appendAppNote(res.session.id, `[session] ${escapeForTerminal(res.session.label)}\r\n`);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  /** Asks first when a command is running; used by the tab X and both close chords. */
  function requestCloseSession(sessionId: string) {
    if (
      sessionIsRunning(sessionId) &&
      !window.confirm("A command is still running in this session. Close it and stop the command?")
    ) {
      return;
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
      setError(e instanceof Error ? e.message : String(e));
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
    if (
      (cwdDiffers || item.source === "semantic") &&
      !window.confirm(
        `Run again in "${session.label}" (${session.cwd})?\n\n${escapeForTerminal(command)}` +
          (cwdDiffers ? `\n\nIt first ran in ${item.cwd}.` : ""),
      )
    ) {
      return;
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
      setError(e instanceof Error ? e.message : String(e));
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
      setPlanNotice(`This plan ${original}. It will run in "${live.label}" (${live.cwd}) instead.`);
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
      projectRoot: session.cwd,
      createdAt: new Date().toISOString(),
    };

    try {
      await workflowAdd({ workflow });
    } catch (e: unknown) {
      setError(`Could not save the workflow: ${e instanceof Error ? e.message : String(e)}`);
      return;
    }
    addWorkflow(workflow);
    appendAppNote(session.id, `[workflow:saved] ${escapeForTerminal(workflow.label)}\r\n`);
    setHistoryOpen(false);
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
      appendAppNote(sessionId, `[workflow:done] ${escapeForTerminal(run.workflowName)} — ${succeeded}/${total} succeeded${durSuffix}\r\n`);
    } else if (finalStatus === "failed") {
      const failedStep = run.steps.find((s) => s.status === "failed");
      appendAppNote(sessionId, `[workflow:failed] ${escapeForTerminal(run.workflowName)} — ${succeeded}/${total} succeeded, failed on step ${(failedStep?.index ?? 0) + 1}${durSuffix}\r\n`);
    } else {
      const skipped = run.steps.filter((s) => s.status === "skipped").length;
      const interruptedStep = run.steps.find((s) => s.status === "interrupted");
      appendAppNote(sessionId, `[workflow:interrupted] ${escapeForTerminal(run.workflowName)} — interrupted during step ${(interruptedStep?.index ?? 0) + 1}; ${skipped} skipped${durSuffix}\r\n`);
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
    if (
      workflow.projectRoot &&
      workflow.projectRoot !== session.cwd &&
      !window.confirm(
        `Run "${escapeForTerminal(workflow.label)}" in "${session.label}" (${session.cwd})? It was saved for ${workflow.projectRoot}.\n\n${stepCommands.map(escapeForTerminal).join("\n")}`,
      )
    ) {
      return;
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
            `[workflow:waiting] step ${i + 1}/${commands.length} is still running. Use Interrupt to stop it.`,
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
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (workflowAbortBySessionRef.current.get(runSessionId) === controller) {
        workflowAbortBySessionRef.current.delete(runSessionId);
      }
      unlockSession(runSessionId);
    }
  }

  // --- Workflow delete handler ---
  async function handleDeleteWorkflow(workflowId: string) {
    try {
      await workflowDelete({ id: workflowId });
      removeWorkflow(workflowId);
    } catch (e: unknown) {
      // The row never reached the database: it can only be removed from the UI.
      if (isMissingIdError(e)) {
        removeWorkflow(workflowId);
        return;
      }
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // --- Cross-drawer navigation (Phase 6D) ---

  function handleViewWorkflowRun(workflowRunId: string) {
    // Find which workflow owns this run
    const entry = Object.entries(lastRunByWorkflowId).find(
      ([, run]) => run.id === workflowRunId,
    );
    if (!entry) return;
    const [workflowId] = entry;
    setHistoryOpen(false);
    setWorkflowOpen(true);
    setExpandedRunWorkflowId(workflowId);
  }

  function handleRetryFailedStep(command: string) {
    setInputValue(command);
    setWorkflowOpen(false);
    setExpandedRunWorkflowId(null);
    requestAnimationFrame(() => composerRef.current?.focus());
  }

  function handleViewHistoryItemFromRun(historyItemId: string) {
    setWorkflowOpen(false);
    setExpandedRunWorkflowId(null);
    setHistoryInitialExpandedId(historyItemId);
    setHistoryOpen(true);
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
      if (isMissingIdError(e)) {
        // The database no longer holds it as pending (accepted or dismissed earlier, or never
        // stored). Stop offering it, but say plainly that nothing was created: an explicit
        // Accept must not read as success or as a quiet dismissal.
        setMemorySuggestions((prev) =>
          prev.map((s) =>
            s.id === suggestionId ? { ...s, status: "dismissed" as const } : s,
          ),
        );
        setError(
          "That suggestion is no longer pending in the database (it was accepted or dismissed before), so no memory item was created and it was removed from the list.",
        );
        return;
      }
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  const confirmingRef = useRef(false);
  async function handleWorkflowEditorConfirm(label: string, steps: string[]) {
    if (!workflowEditorData || confirmingRef.current) return;
    confirmingRef.current = true;
    const { workflowId, suggestionId, projectRoot } = workflowEditorData;

    try {
      // 1. Create and persist the workflow first. The editor stays open until this
      //    succeeds so the user's edits are never lost to a failed write.
      const workflow: Workflow = {
        id: workflowId,
        label,
        source: "promoted",
        command: steps.join(" && "),
        steps: steps.map((cmd) => ({ command: cmd })),
        projectRoot,
        createdAt: new Date().toISOString(),
      };
      await workflowAdd({ workflow });
      addWorkflow(workflow);
      setWorkflowEditorData(null);
      if (session) {
        appendAppNote(session.id, `[workflow:promoted] ${escapeForTerminal(workflow.label)}\r\n`);
      }

      // 2. Accept the memory suggestion. A failure here is partial success: the
      //    workflow exists, so report it instead of discarding anything.
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
        if (isMissingIdError(acceptErr)) {
          setMemorySuggestions((prev) =>
            prev.map((s) =>
              s.id === suggestionId ? { ...s, status: "accepted" as const } : s,
            ),
          );
        } else {
          setError(
            `Workflow saved, but the suggestion could not be marked accepted: ${acceptErr instanceof Error ? acceptErr.message : String(acceptErr)}`,
          );
        }
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
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
      if (!isMissingIdError(e)) {
        setError(e instanceof Error ? e.message : String(e));
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
    try {
      await memoryDelete({ memoryId });
      removeMemoryItem(memoryId);
    } catch (e: unknown) {
      if (isMissingIdError(e)) {
        removeMemoryItem(memoryId);
        return;
      }
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // The backend reports an id it does not hold (or a suggestion no longer pending) as an error.
  // For the UI that is the state the user asked for, so callers reconcile instead of looping on it.
  function isMissingIdError(e: unknown): boolean {
    const msg = e instanceof Error ? e.message : String(e);
    return /not found|not pending|no suggestion|no memory item|no workflow/i.test(msg);
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

  const showPlanColumn =
    productMode === "guided" || plan !== null;

  const displayExplanation =
    plan && simplifiedSummaries
      ? simplifyText(plan.plan.explanation)
      : plan?.plan.explanation ?? "";

  // --- Boot failure screen ---
  if (bootPhase === "failed") {
    return (
      <div className="app-shell">
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
            If this persists, check that your Tauri backend is running.
          </p>
        </div>
      </div>
    );
  }

  // --- Render ---
  return (
    <div className="app-shell">
      <header className="topbar">
        <div>
          <strong>CommandUI</strong>
          <span className="muted"> v{APP_VERSION}</span>
          {session && (
            <span className="muted"> — {session.cwd ?? session.label}</span>
          )}
        </div>
        <div className="topbar-actions">
          <button type="button" onClick={() => setHistoryOpen(true)}>
            History
          </button>
          <button type="button" onClick={() => setWorkflowOpen(true)}>
            Workflows
          </button>
          <button type="button" onClick={() => setMemoryOpen(true)}>
            Memory
          </button>
          <button type="button" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
        </div>
      </header>

      {browserPreview && (
        <div className="preview-banner">
          Browser preview mode — backend commands disabled. Run{" "}
          <code>pnpm tauri:dev</code> for the full experience.
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
            <div className="desync-banner">
              <span>Terminal appears desynced.</span>
              <button type="button" onClick={handleResync}>
                Resync
              </button>
            </div>
          )}

          {activeBootStalled && !activeExited && activeSessionId && (
            <div className="desync-banner" role="status">
              <span>
                The {session?.shell ?? "shell"} in this session has not reported ready.
                A shell profile that errors or replaces the prompt can cause this.
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
            <div className="desync-banner" role="status">
              <span>
                {activeExecState === "interrupting"
                  ? "The interrupt has not ended the command."
                  : "A command you typed has held this session for a while."}{" "}
                A nested shell, ssh session or REPL stays open until you type exit in the terminal.
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

          <TerminalPane
            ref={terminalPaneRef}
            sessionId={activeSessionId}
            executionStatus={visibleExecutionStatus}
            statusLabel={activeExecState === "userRunning" ? "running (typed)" : undefined}
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
            <div className="error-box">
              <span>{error}</span>
              <button type="button" onClick={() => setError(null)}>
                Dismiss
              </button>
            </div>
          )}

          {!reducedClutter && (
            <MemorySuggestions
              suggestions={memorySuggestions.filter(
                (s) => s.status === "pending",
              )}
              onAccept={handleAcceptSuggestion}
              onDismiss={handleDismissSuggestion}
            />
          )}

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
                ? "Shell exited — open a new session."
                : activeExecState === "desynced"
                  ? "Terminal out of sync — use Resync."
                  : activeBootStalled
                    ? "Terminal not ready — use Resync or close the session."
                    : activeExecState === "booting"
                      ? "Terminal starting…"
                      : activeExecState === "userRunning"
                        ? "A command you typed is running — wait for the prompt."
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
                requireMediumRiskConfirmation={confirmMediumRisk}
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
            ) : (
              <div className="plan-panel">
                <p className="muted">No semantic plan yet.</p>
              </div>
            )}
          </aside>
        )}
      </main>

      <HistoryDrawer
        isOpen={historyOpen}
        items={visibleHistoryItems}
        allItems={historyItems}
        sessions={sessions}
        activeSessionId={activeSessionId}
        onClose={() => {
          setHistoryOpen(false);
          setHistoryInitialExpandedId(null);
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        onRerun={handleRerunHistoryItem}
        onReopenPlan={handleReopenPlan}
        onSaveWorkflow={handleSaveWorkflowFromHistory}
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
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        onRun={handleRunWorkflow}
        onDelete={handleDeleteWorkflow}
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
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        onDelete={handleDeleteMemory}
        loading={bootPhase === "booting"}
      />

      <CommandPalette
        isOpen={paletteOpen}
        onClose={() => {
          setPaletteOpen(false);
          requestAnimationFrame(() => {
            restorePreviousZone();
            composerRef.current?.focus();
          });
        }}
        actions={paletteActions}
      />

      <SettingsDrawer
        isOpen={settingsOpen}
        onClose={() => {
          setSettingsOpen(false);
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        productMode={productMode}
        onProductModeChange={setProductMode}
        defaultInputMode={defaultInputMode}
        onDefaultInputModeChange={setDefaultInputMode}
        reducedClutter={reducedClutter}
        onReducedClutterChange={setReducedClutter}
        simplifiedSummaries={simplifiedSummaries}
        onSimplifiedSummariesChange={setSimplifiedSummaries}
        confirmMediumRisk={confirmMediumRisk}
        onConfirmMediumRiskChange={setConfirmMediumRisk}
      />

      {workflowEditorData && (
        <WorkflowEditor
          initialLabel={workflowEditorData.label}
          initialSteps={workflowEditorData.steps}
          projectRoot={workflowEditorData.projectRoot}
          onConfirm={handleWorkflowEditorConfirm}
          onCancel={handleWorkflowEditorCancel}
        />
      )}
    </div>
  );
}
