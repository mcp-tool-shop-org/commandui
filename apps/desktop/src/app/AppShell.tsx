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
} from "../features/memory/memoryClient";
import { InputComposer } from "../components/InputComposer";
import type { InputComposerHandle } from "../components/InputComposer";
import { PlanPanel, planCanRun } from "../components/PlanPanel";
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

const APP_VERSION = "1.0.2";
const SESSION_BUSY_MESSAGE = "A command is already running in this session.";
const SESSION_NOT_READY_MESSAGE =
  "The terminal is not ready yet. Wait for the session to finish starting, or resync it.";
/** Per-session replay buffer cap; xterm keeps its own scrollback. */
const TERMINAL_REPLAY_MAX_LINES = 5000;
const TERMINAL_REPLAY_TRUNCATED_MARKER = "[earlier output truncated]\r\n";

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

  const acceptPlanRunGate = useCallback((gate: PlanRunGate) => {
    planGateRef.current = gate;
  }, []);

  // Keep ref in sync with state (for use in event callbacks)
  activeSessionIdRef.current = activeSessionId;

  // A new plan (including a reopen of the same command) starts unconfirmed.
  if (planNonceSeenRef.current !== planNonce || plan === null) {
    planNonceSeenRef.current = planNonce;
    planGateRef.current = { command: "", confirmed: false };
  }

  const session =
    sessions.find((s) => s.id === activeSessionId) ?? null;

  const activeExecState: SessionExecState =
    (activeSessionId ? sessionExecStates[activeSessionId] : undefined) ?? "booting";
  const activeBadge: SessionBadgeStatus =
    (activeSessionId && sessionBadge[activeSessionId]) || "idle";
  const isRunning =
    activeBadge === "running" ||
    activeExecState === "running" ||
    activeExecState === "interrupting";
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
    const exec = useExecutionStore.getState().sessionExecStates[sessionId];
    return exec === "running" || exec === "interrupting";
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
    if (sid) delete terminalLinesBySessionRef.current[sid];
    terminalPaneRef.current?.clear();
  }

  function sessionNotReady(sessionId: string): boolean {
    const exec = useExecutionStore.getState().sessionExecStates[sessionId] ?? "booting";
    return exec === "booting" || exec === "desynced";
  }

  function appendTerminalLine(sessionId: string, line: string) {
    const clutter = useSettingsStore.getState().reducedClutter;
    if (clutter && (line.startsWith("[exec:") || line.startsWith("[active]"))) return;

    // Store in background buffer
    const buffers = terminalLinesBySessionRef.current;
    const buffer = buffers[sessionId] ?? (buffers[sessionId] = []);
    buffer.push(line);
    if (buffer.length > TERMINAL_REPLAY_MAX_LINES) {
      buffer.splice(0, buffer.length - TERMINAL_REPLAY_MAX_LINES + 1, TERMINAL_REPLAY_TRUNCATED_MARKER);
    }

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
        } else {
          const res = await createSession({ label: "Session 1" });
          addSession(res.session);
          appendTerminalLine(
            res.session.id,
            `Welcome to CommandUI — ${res.session.label}\r\n`,
          );
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
        } catch {
          // memory not critical
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
    setSessionExecState(sessionId, "running");
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
            setSessionExecState(event.sessionId, "ready");
          },
        ),
        onMockEvent<{ sessionId: string; execState: SessionExecState }>(
          "session:exec_state_changed",
          (event) => {
            setSessionExecState(event.sessionId, event.execState);
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
      setSessionExecState(event.sessionId, "ready");
    }));
    remember(subscribeToExecStateChanged((event) => {
      setSessionExecState(event.sessionId, event.execState);
    }));

    return () => {
      disposed = true;
      for (const subscription of pending) {
        void subscription.then((unlisten) => {
          unlisten();
        });
      }
    };
    // Rebind when reduced clutter changes so the line filter is not stuck on the first render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reducedClutter]);

  // --- Replay buffer on session switch ---
  useEffect(() => {
    if (!activeSessionId) return;
    const pane = terminalPaneRef.current;
    if (!pane) return;

    pane.clear();
    const buffer = terminalLinesBySessionRef.current[activeSessionId] ?? [];
    for (const line of buffer) {
      pane.write(line);
    }
  }, [activeSessionId]); // eslint-disable-line react-hooks/exhaustive-deps

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
    if (overlay.planOpen) {
      handleRejectPlan();
    }
  }

  function focusComposer() {
    composerRef.current?.focus();
  }

  const shortcuts = useMemo<ShortcutDef[]>(() => {
    const defs: ShortcutDef[] = [
      { id: "palette",       combo: "ctrl+k",       context: ["global"], action: () => setPaletteOpen(true) },
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
      { id: "plan-approve-global", combo: "ctrl+enter", context: ["global"], when: () => plan !== null, action: () => approvePlanRef.current(planGateRef.current.command) },
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

    // Ctrl+W close session only in Tauri mode (conflicts with browser tab close)
    if (!browserPreview) {
      defs.push({
        id: "close-session",
        combo: "ctrl+w",
        context: ["global"],
        action: () => activeSessionId && handleCloseSession(activeSessionId),
      });
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
    if (activeExecState === "desynced") {
      actions.push({ id: "resync", label: "Resync Terminal", action: handleResync });
    }

    return actions;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessions, isRunning, activeExecState]);

  // execute() rejected before any ExecutionFinished event: close the row out.
  function failRejectedExecution(historyId: string, err: unknown) {
    const finishedAt = new Date().toISOString();
    updateHistoryItem(historyId, { status: "failure", finishedAt });
    persistInBackground("history update", historyUpdate({ historyId, status: "failure", finishedAt }));
    void err;
  }

  // --- Submit handler ---
  async function handleSubmit(value: string) {
    if (!session) return;
    if (busySessionsRef.current.has(session.id)) return;
    if (inputMode === "command" && sessionIsRunning(session.id)) {
      setError(SESSION_BUSY_MESSAGE);
      return;
    }
    const sessionId = session.id;
    if (inputMode === "command" && sessionNotReady(sessionId)) {
      setError(SESSION_NOT_READY_MESSAGE);
      return;
    }
    lockSession(sessionId);
    setError(null);

    try {
      if (inputMode === "command") {
        // --- Raw command flow ---
        if (sessionIsRunning(sessionId)) {
          setError(SESSION_BUSY_MESSAGE);
          return;
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

        planGateRef.current = { command: "", confirmed: false };
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

        appendTerminalLine(session.id, `? ${value}\r\n`);
        appendTerminalLine(session.id, `[plan] ${res.plan.command}\r\n`);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      unlockSession(sessionId);
    }
  }

  // --- Plan actions ---
  async function handleApprovePlan(approvedCommand: string) {
    if (!session || !plan) return;
    if (sessionIsRunning(session.id) || busySessionsRef.current.has(session.id)) {
      setError(SESSION_BUSY_MESSAGE);
      return;
    }
    if (sessionNotReady(session.id)) {
      setError(SESSION_NOT_READY_MESSAGE);
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
      })
    ) {
      return;
    }
    const sessionId = session.id;
    lockSession(sessionId);

    try {
      if (sessionIsRunning(sessionId)) {
        setError(SESSION_BUSY_MESSAGE);
        return;
      }
      inFlightExecRef.current.add(sessionId);
      const executionId = crypto.randomUUID();

      // A plan opened from history has no live row (currentPlanHistoryId is
      // null): the run gets a NEW row and the original outcome is untouched.
      let runHistoryId: string;
      let createdRunRow = false;
      if (currentPlanHistoryId) {
        runHistoryId = currentPlanHistoryId;
        updateHistoryItem(runHistoryId, { executedCommand: trimmed });
        persistInBackground("history update", historyUpdate({ historyId: runHistoryId, executedCommand: trimmed }));
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

      appendTerminalLine(session.id, `[approved] ${trimmed}\r\n`);

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
          updateHistoryItem(runHistoryId, { status: "failure", executedCommand: undefined, finishedAt });
          persistInBackground("history update", historyUpdate({ historyId: runHistoryId, status: "failure", executedCommand: undefined, finishedAt }));
        }
        throw err;
      } finally {
        inFlightExecRef.current.delete(sessionId);
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
      setCurrentPlanHistoryId(null);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      unlockSession(sessionId);
    }
  }

  approvePlanRef.current = (command: string) => {
    void handleApprovePlan(command);
  };

  function handleRejectPlan() {
    if (!session) return;

    if (currentPlanHistoryId) {
      updateHistoryItem(currentPlanHistoryId, { status: "rejected" });
      persistInBackground("history update", historyUpdate({
        historyId: currentPlanHistoryId,
        status: "rejected",
      }));
    }

    appendTerminalLine(session.id, "[rejected]\r\n");
    setPlan(null);
    setCurrentPlanHistoryId(null);
  }

  function handleSaveWorkflow(command: string) {
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

    addWorkflow(workflow);
    persistInBackground("workflow add", workflowAdd({ workflow }));
    appendTerminalLine(session.id, `[workflow:saved] ${workflow.label}\r\n`);
  }

  // --- Terminal handlers ---
  const handleTerminalData = useCallback(
    (data: string) => {
      if (!activeSessionId) return;
      void writeTerminal({ sessionId: activeSessionId, data });
    },
    [activeSessionId],
  );

  const handleTerminalResize = useCallback(
    (cols: number, rows: number) => {
      if (!activeSessionId) return;
      void resizeTerminal({ sessionId: activeSessionId, cols, rows });
    },
    [activeSessionId],
  );

  // --- Interrupt handler ---
  async function handleInterrupt() {
    const sessionId = useSessionStore.getState().activeSessionId;
    if (!sessionId || !sessionIsRunning(sessionId)) return;
    try {
      await interruptTerminal({ sessionId });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // --- Resync handler ---
  async function handleResync() {
    if (!activeSessionId) return;
    try {
      await resyncTerminal({ sessionId: activeSessionId });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // --- Session handlers ---
  async function handleCreateSession() {
    try {
      const label = `Session ${sessions.length + 1}`;
      const res = await createSession({ label });
      addSession(res.session);
      setActiveSessionId(res.session.id);
      appendTerminalLine(res.session.id, `[session] ${res.session.label}\r\n`);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function handleCloseSession(sessionId: string) {
    workflowAbortBySessionRef.current.get(sessionId)?.abort();
    try {
      await closeSession({ sessionId });
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
    if (sessionIsRunning(session.id) || busySessionsRef.current.has(session.id)) {
      setError(SESSION_BUSY_MESSAGE);
      return;
    }

    if (sessionNotReady(session.id)) {
      setError(SESSION_NOT_READY_MESSAGE);
      return;
    }

    const sessionId = session.id;
    lockSession(sessionId);
    try {
      if (sessionIsRunning(sessionId)) {
        setError(SESSION_BUSY_MESSAGE);
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

    const syntheticPlan: CommandPlan = {
      id: item.linkedPlanId ?? crypto.randomUUID(),
      sessionId: item.sessionId,
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

  function handleSaveWorkflowFromHistory(item: HistoryItem) {
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

    addWorkflow(workflow);
    persistInBackground("workflow add", workflowAdd({ workflow }));
    appendTerminalLine(session.id, `[workflow:saved] ${workflow.label}\r\n`);
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
      appendTerminalLine(sessionId, `[workflow:done] ${run.workflowName} — ${succeeded}/${total} succeeded${durSuffix}\r\n`);
    } else if (finalStatus === "failed") {
      const failedStep = run.steps.find((s) => s.status === "failed");
      appendTerminalLine(sessionId, `[workflow:failed] ${run.workflowName} — ${succeeded}/${total} succeeded, failed on step ${(failedStep?.index ?? 0) + 1}${durSuffix}\r\n`);
    } else {
      const skipped = run.steps.filter((s) => s.status === "skipped").length;
      const interruptedStep = run.steps.find((s) => s.status === "interrupted");
      appendTerminalLine(sessionId, `[workflow:interrupted] ${run.workflowName} — interrupted during step ${(interruptedStep?.index ?? 0) + 1}; ${skipped} skipped${durSuffix}\r\n`);
    }
  }

  // --- Workflow run handler ---
  async function handleRunWorkflow(workflow: Workflow) {
    if (!session) return;
    const runSessionId = session.id;
    if (sessionIsRunning(runSessionId) || busySessionsRef.current.has(runSessionId)) {
      setError(SESSION_BUSY_MESSAGE);
      return;
    }
    if (sessionNotReady(runSessionId)) {
      setError(SESSION_NOT_READY_MESSAGE);
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
          setError(SESSION_BUSY_MESSAGE);
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

        const waited = await waitForStepCompletion(executionId, controller.signal);
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
          if (waited.reason === "timeout") {
            // The command may still be running in the PTY. Stop it and leave the
            // local running state to execution_finished instead of writing Ready.
            try {
              await interruptTerminal({ sessionId: runSessionId });
            } catch {
              // surfaced through the error below
            }
            setError(
              "Workflow step timed out; the running command was interrupted. The session stays busy until the terminal reports it finished.",
            );
          } else {
            clearSessionRunning(runSessionId);
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
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  const confirmingRef = useRef(false);
  async function handleWorkflowEditorConfirm(label: string, steps: string[]) {
    if (!workflowEditorData || confirmingRef.current) return;
    confirmingRef.current = true;
    const { workflowId, suggestionId, projectRoot } = workflowEditorData;
    setWorkflowEditorData(null);

    try {
      // 1. Accept the memory suggestion
      const res = await memoryAcceptSuggestion({ suggestionId });
      if (res.createdItem) {
        addMemoryItem(res.createdItem);
      }
      setMemorySuggestions((prev) =>
        prev.map((s) =>
          s.id === suggestionId ? { ...s, status: "accepted" as const } : s,
        ),
      );

      // 2. Create and persist the workflow with edited data
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
      if (session) {
        appendTerminalLine(session.id, `[workflow:promoted] ${workflow.label}\r\n`);
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
      // Mark as dismissed in store (don't remove — detectors need dismissed state to avoid re-suggesting)
      setMemorySuggestions((prev) =>
        prev.map((s) =>
          s.id === suggestionId ? { ...s, status: "dismissed" as const } : s,
        ),
      );
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function handleDeleteMemory(memoryId: string) {
    try {
      await memoryDelete({ memoryId });
      removeMemoryItem(memoryId);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
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

    for (const candidate of candidates) {
      try {
        await memoryStoreSuggestion({ suggestion: candidate });
      } catch {
        // persistence not critical
      }
    }

    if (candidates.length > 0) {
      // Use function-form setter so the store's built-in dedup runs on latest state
      setMemorySuggestions((prev) => [...prev, ...candidates]);
    }
  }

  // --- Plan display ---
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
            onClose={handleCloseSession}
          />

          {activeExecState === "desynced" && (
            <div className="desync-banner">
              <span>Terminal appears desynced.</span>
              <button type="button" onClick={handleResync}>
                Resync
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
            onResize={handleTerminalResize}
            onData={handleTerminalData}
            autoFocus
          />

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
            disabled={activeExecState !== "ready"}
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
      />

      <MemoryDrawer
        isOpen={memoryOpen}
        items={memoryItems}
        onClose={() => {
          setMemoryOpen(false);
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        onDelete={handleDeleteMemory}
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
