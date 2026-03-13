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
import type { PlannerGeneratePlanResponse, SessionExecState } from "@commandui/api-contract";
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
import { PlanPanel } from "../components/PlanPanel";
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

const APP_VERSION = "0.0.1";

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
    executionStatus,
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
  const [currentPlanHistoryId, setCurrentPlanHistoryId] = useState<
    string | null
  >(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
  const [terminalLinesBySession, setTerminalLinesBySession] = useState<
    Record<string, string[]>
  >({});
  const executionToHistoryRef = useRef<Record<string, string>>({});
  const bootedRef = useRef(false);
  const terminalPaneRef = useRef<TerminalPaneHandle>(null);
  const composerRef = useRef<InputComposerHandle>(null);
  const activeSessionIdRef = useRef<string | null>(null);

  // Keep ref in sync with state (for use in event callbacks)
  activeSessionIdRef.current = activeSessionId;

  const session =
    sessions.find((s) => s.id === activeSessionId) ?? null;

  const activeExecState: SessionExecState =
    (activeSessionId ? sessionExecStates[activeSessionId] : undefined) ?? "booting";
  const isRunning = executionStatus === "running";

  const visibleHistoryItems = activeSessionId
    ? historyItems.filter((h) => h.sessionId === activeSessionId)
    : historyItems;

  // --- Helpers ---
  function appendTerminalLine(sessionId: string, line: string) {
    if (reducedClutter && (line.startsWith("[exec:") || line.startsWith("[active]"))) return;

    // Store in background buffer
    setTerminalLinesBySession((prev) => ({
      ...prev,
      [sessionId]: [...(prev[sessionId] ?? []), line],
    }));

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
        try {
          const settingsRes = await settingsGet();
          if (settingsRes.settings) {
            const s = settingsRes.settings as Record<string, unknown>;
            if (typeof s.productMode === "string") setProductMode(s.productMode as "classic" | "guided");
            if (typeof s.defaultInputMode === "string") setDefaultInputMode(s.defaultInputMode as "command" | "ask");
            if (typeof s.reducedClutter === "boolean") setReducedClutter(s.reducedClutter);
            if (typeof s.simplifiedSummaries === "boolean") setSimplifiedSummaries(s.simplifiedSummaries);
            if (typeof s.confirmMediumRisk === "boolean") setConfirmMediumRisk(s.confirmMediumRisk);
          }
        } catch {
          // settings not critical
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
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
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

  // --- Terminal event subscriptions ---
  useEffect(() => {
    if (browserPreview) {
      // Use mock event bus in browser preview mode
      const unlisteners = [
        onMockEvent<{ sessionId: string; executionId?: string; text: string }>(
          "terminal:line",
          (event) => appendTerminalLine(event.sessionId, event.text),
        ),
        onMockEvent<{ execution: { id: string } }>(
          "terminal:execution_started",
          (event) => {
            setActiveExecution(event.execution.id);
            setExecutionStatus("running");
          },
        ),
        onMockEvent<{ executionId: string; status: string; exitCode: number }>(
          "terminal:execution_finished",
          (event) => {
            setActiveExecution(null);
            setLastExecutionId(event.executionId);
            const status = event.status as "success" | "failure" | "interrupted";
            setExecutionStatus(status === "interrupted" ? "idle" : status);

            const historyId =
              executionToHistoryRef.current[event.executionId] ?? event.executionId;

            const finishedAt = new Date().toISOString();
            const found = useHistoryStore.getState().items.find((h) => h.id === historyId);
            const durationMs = found
              ? Date.now() - new Date(found.createdAt).getTime()
              : undefined;

            updateHistoryItem(historyId, { status, exitCode: event.exitCode, finishedAt, durationMs });
            void historyUpdate({ historyId, status, exitCode: event.exitCode, finishedAt, durationMs });

            // Run pattern detectors after execution completes
            void generateSuggestions();
          },
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
      return () => { for (const u of unlisteners) u(); };
    }

    // Tauri runtime — use real event listeners
    const unlisteners: Array<() => void> = [];

    subscribeToTerminalLines((event) => {
      appendTerminalLine(event.sessionId, event.text);
    }).then((u) => unlisteners.push(u));

    subscribeToExecutionStarted((event) => {
      setActiveExecution(event.execution.id);
      setExecutionStatus("running");
    }).then((u) => unlisteners.push(u));

    subscribeToExecutionFinished((event) => {
      setActiveExecution(null);
      setLastExecutionId(event.executionId);
      const status = event.status;
      setExecutionStatus(status === "interrupted" ? "idle" : status);

      const historyId =
        executionToHistoryRef.current[event.executionId] ?? event.executionId;

      // Compute duration from createdAt
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

      void historyUpdate({
        historyId,
        status,
        exitCode: event.exitCode,
        finishedAt,
        durationMs,
      });

      // Run pattern detectors after execution completes
      void generateSuggestions();
    }).then((u) => unlisteners.push(u));

    subscribeToSessionCwdChanged((event) => {
      updateSession(event.sessionId, { cwd: event.cwd });
    }).then((u) => unlisteners.push(u));

    subscribeToSessionReady((event) => {
      setSessionExecState(event.sessionId, "ready");
    }).then((u) => unlisteners.push(u));

    subscribeToExecStateChanged((event) => {
      setSessionExecState(event.sessionId, event.execState);
    }).then((u) => unlisteners.push(u));

    return () => {
      for (const u of unlisteners) u();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // --- Replay buffer on session switch ---
  useEffect(() => {
    if (!activeSessionId) return;
    const pane = terminalPaneRef.current;
    if (!pane) return;

    pane.clear();
    const buffer = terminalLinesBySession[activeSessionId] ?? [];
    for (const line of buffer) {
      pane.write(line);
    }
  }, [activeSessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  // --- Settings persistence ---
  useEffect(() => {
    if (browserPreview) return;
    void settingsUpdate({
      settings: {
        productMode,
        reducedClutter,
        simplifiedSummaries,
        confirmMediumRisk,
        defaultInputMode,
      },
    });
  }, [productMode, reducedClutter, simplifiedSummaries, confirmMediumRisk, defaultInputMode]);

  // --- Centralized keyboard shortcuts ---
  const anyDrawerOpen = historyOpen || workflowOpen || memoryOpen || settingsOpen;

  function closeAllOverlays() {
    if (workflowEditorData) { setWorkflowEditorData(null); return; }
    if (paletteOpen) { setPaletteOpen(false); return; }
    if (anyDrawerOpen) {
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
    if (plan) {
      handleRejectPlan();
      return;
    }
  }

  function focusComposer() {
    composerRef.current?.focus();
  }

  const shortcuts = useMemo<ShortcutDef[]>(() => {
    const defs: ShortcutDef[] = [
      { id: "palette",       combo: "ctrl+k",       context: ["global"], action: () => setPaletteOpen(true) },
      { id: "focus-composer", combo: "ctrl+j",       context: ["global"], action: focusComposer },
      { id: "clear-terminal", combo: "ctrl+l",       context: ["global"], action: () => terminalPaneRef.current?.clear() },
      { id: "new-session",   combo: "ctrl+t",        context: ["global"], action: handleCreateSession },
      { id: "history",       combo: "ctrl+h",        context: ["global"], action: () => setHistoryOpen((v) => !v) },
      { id: "workflows",     combo: "ctrl+shift+w",  context: ["global"], action: () => setWorkflowOpen((v) => !v) },
      { id: "memory",        combo: "ctrl+m",        context: ["global"], action: () => setMemoryOpen((v) => !v) },
      { id: "settings",      combo: "ctrl+,",        context: ["global"], action: () => setSettingsOpen((v) => !v) },
      { id: "escape",        combo: "escape",        context: ["global"], action: closeAllOverlays },
      // Plan shortcuts (zone-specific, bare keys only fire when plan is focused)
      { id: "plan-approve",  combo: "a",             context: ["plan"],   when: () => plan !== null, action: () => plan && handleApprovePlan(plan.plan.command) },
      { id: "plan-approve-global", combo: "ctrl+enter", context: ["global"], when: () => plan !== null, action: () => plan && handleApprovePlan(plan.plan.command) },
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
  }, [sessions, activeSessionId, plan, browserPreview]);

  useShortcuts(shortcuts);

  // --- Command palette actions ---
  const paletteActions = useMemo<PaletteAction[]>(() => {
    const actions: PaletteAction[] = [
      { id: "new-session",    label: "New Session",        shortcut: "Ctrl+T",       action: handleCreateSession },
      { id: "focus-composer",  label: "Focus Composer",    shortcut: "Ctrl+J",       action: focusComposer },
      { id: "clear-terminal",  label: "Clear Terminal",    shortcut: "Ctrl+L",       action: () => terminalPaneRef.current?.clear() },
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

  // --- Submit handler ---
  async function handleSubmit(value: string) {
    if (!session || busy) return;
    setBusy(true);
    setError(null);

    try {
      if (inputMode === "command") {
        // --- Raw command flow ---
        const executionId = crypto.randomUUID();
        const historyItem: HistoryItem = {
          id: executionId,
          sessionId: session.id,
          source: "raw",
          userInput: value,
          executedCommand: value,
          status: "planned",
          createdAt: new Date().toISOString(),
          cwd: session.cwd,
        };
        appendHistoryItem(historyItem);
        executionToHistoryRef.current[executionId] = executionId;

        void historyAppend({ item: historyItem });

        await executeCommand({
          executionId,
          sessionId: session.id,
          command: value,
          source: "raw",
        });
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

        setPlan(res);
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

        void historyAppend({ item: historyItem });
        void planStore({ plan: res.plan });

        appendTerminalLine(session.id, `? ${value}\r\n`);
        appendTerminalLine(session.id, `[plan] ${res.plan.command}\r\n`);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setBusy(false);
    }
  }

  // --- Plan actions ---
  async function handleApprovePlan(approvedCommand: string) {
    if (!session || !plan) return;
    setBusy(true);

    try {
      const executionId = crypto.randomUUID();

      if (currentPlanHistoryId) {
        updateHistoryItem(currentPlanHistoryId, {
          executedCommand: approvedCommand,
        });
        executionToHistoryRef.current[executionId] = currentPlanHistoryId;

        void historyUpdate({
          historyId: currentPlanHistoryId,
          executedCommand: approvedCommand,
        });
      }

      // Check for edit-based memory suggestion
      if (
        approvedCommand !== plan.plan.command &&
        session.cwd
      ) {
        const existing = memorySuggestions.find(
          (s) =>
            s.kind === "accepted_substitution" &&
            s.proposedValue === approvedCommand &&
            s.projectRoot === session.cwd,
        );

        if (!existing) {
          const suggestion: MemorySuggestion = {
            id: crypto.randomUUID(),
            scope: "project",
            projectRoot: session.cwd,
            kind: "accepted_substitution",
            label: `Use "${approvedCommand}" instead of "${plan.plan.command}"`,
            proposedKey: plan.plan.command,
            proposedValue: approvedCommand,
            confidence: 0.72,
            derivedFromHistoryIds: currentPlanHistoryId
              ? [currentPlanHistoryId]
              : [],
            status: "pending",
            createdAt: new Date().toISOString(),
          };
          setMemorySuggestions((prev) => [suggestion, ...prev]);
        }
      }

      appendTerminalLine(session.id, `[approved] ${approvedCommand}\r\n`);

      await executeCommand({
        executionId,
        sessionId: session.id,
        command: approvedCommand,
        source: "semantic",
        linkedPlanId: plan.plan.id,
      });

      setPlan(null);
      setCurrentPlanHistoryId(null);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setBusy(false);
    }
  }

  function handleRejectPlan() {
    if (!session) return;

    if (currentPlanHistoryId) {
      updateHistoryItem(currentPlanHistoryId, { status: "rejected" });
      void historyUpdate({
        historyId: currentPlanHistoryId,
        status: "rejected",
      });
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
    void workflowAdd({ workflow });
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
    if (!activeSessionId) return;
    try {
      await interruptTerminal({ sessionId: activeSessionId });
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
    try {
      await closeSession({ sessionId });
      removeSession(sessionId);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // --- History action handlers ---
  async function handleRerunHistoryItem(item: HistoryItem) {
    if (!session) return;
    const command = item.executedCommand ?? item.generatedCommand;
    if (!command) return;

    setBusy(true);
    try {
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
      void historyAppend({ item: historyItem });

      await executeCommand({
        executionId,
        sessionId: session.id,
        command,
        source: item.source,
        linkedPlanId: item.linkedPlanId,
      });

      setHistoryOpen(false);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
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
      explanation: "Reopened from history",
      assumptions: [],
      confidence: 0.9,
      risk: "low",
      destructive: false,
      requiresConfirmation: false,
      touchesFiles: false,
      touchesNetwork: false,
      escalatesPrivileges: false,
      generatedAt: item.createdAt,
    };

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
    setCurrentPlanHistoryId(item.id);
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
    void workflowAdd({ workflow });
    appendTerminalLine(session.id, `[workflow:saved] ${workflow.label}\r\n`);
    setHistoryOpen(false);
  }

  // --- Workflow run helpers ---
  function waitForStepCompletion(executionId: string): Promise<HistoryItem> {
    return new Promise((resolve) => {
      const check = () => {
        const item = useHistoryStore.getState().items.find((h) => h.id === executionId);
        if (item && item.status !== "planned") resolve(item);
        else setTimeout(check, 100);
      };
      setTimeout(check, 100);
    });
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
    setBusy(true);
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
        const historyItem: HistoryItem = {
          id: executionId,
          sessionId: session.id,
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
        void historyAppend({ item: historyItem });

        await executeCommand({
          executionId,
          sessionId: session.id,
          command: cmd,
          source: historySource,
        });

        // Wait for step completion
        const finished = await waitForStepCompletion(executionId);
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
            writeRunSummary(session.id, { ...latestRun, finishedAt: Date.now() }, mappedStatus === "interrupted" ? "interrupted" : "failed");
          }
          return;
        }
      }

      // All steps succeeded
      const latestRun = useWorkflowRunStore.getState().activeRun;
      completeActiveRun("success");
      if (latestRun) {
        writeRunSummary(session.id, { ...latestRun, finishedAt: Date.now() }, "success");
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
      setBusy(false);
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
        const commands: string[] = JSON.parse(suggestion.proposedValue);
        setWorkflowEditorData({
          workflowId: crypto.randomUUID(),
          suggestionId,
          label: suggestion.proposedKey,
          steps: commands,
          projectRoot: suggestion.projectRoot,
        });
      } catch {
        // invalid JSON — fall through to normal accept
      }
      return;
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
    const session = sessions.find((s) => s.id === activeSessionId);

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

          <TerminalPane
            ref={terminalPaneRef}
            sessionId={activeSessionId}
            executionStatus={executionStatus}
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
            busy={busy}
            isRunning={isRunning}
            onInterrupt={handleInterrupt}
            disabled={activeExecState !== "ready" && activeExecState !== "booting"}
          />
        </section>

        {showPlanColumn && (
          <aside className="plan-column">
            {plan ? (
              <PlanPanel
                sessionId={plan.plan.sessionId}
                intent={plan.plan.userIntent}
                command={plan.plan.command}
                risk={plan.plan.risk}
                explanation={displayExplanation}
                contextSources={plan.review.retrievedContext}
                requireMediumRiskConfirmation={confirmMediumRisk}
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
