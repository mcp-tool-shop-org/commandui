import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  CommandPlan,
  HistoryItem,
  MemorySuggestion,
  SessionSummary,
  Workflow,
} from "@commandui/domain";
import type { PlannerGeneratePlanResponse, SessionExecState } from "@commandui/api-contract";
import {
  useComposerStore,
  useExecutionStore,
  useHistoryStore,
  useSessionStore,
  useMemoryStore,
  useSettingsStore,
  useWorkflowStore,
  useFocusStore,
  resolveEffectiveMemory,
} from "@commandui/state";
import type { ShortcutDef } from "../lib/shortcuts";
import type { ShortcutContext } from "../lib/shortcuts";
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
  workflowList,
  settingsGet,
  settingsUpdate,
} from "../features/persistence/persistenceClient";
import {
  memoryList,
  memoryAcceptSuggestion,
  memoryDismissSuggestion,
  memoryDelete,
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
  const { inputMode, setInputMode } = useComposerStore();
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
  const { items: workflows, setWorkflows, addWorkflow } = useWorkflowStore();
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
  const [memoryOpen, setMemoryOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);

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
        const effectiveMemory = resolveEffectiveMemory(
          memoryItems,
          session.cwd,
        );

        const res = await generatePlan({
          sessionId: session.id,
          userIntent: value,
          context: {
            sessionId: session.id,
            cwd: session.cwd ?? ".",
            projectRoot: session.cwd,
            os: detectOS(),
            shell: session.shell ?? "unknown",
            recentCommands: visibleHistoryItems
              .slice(0, 5)
              .map((h) => h.executedCommand ?? h.userInput),
            memoryItems: effectiveMemory.map((m) => ({
              kind: m.kind,
              key: m.key,
              value: m.value,
              confidence: m.confidence,
            })),
            projectFacts: [],
          },
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

  // --- Workflow run handler ---
  async function handleRunWorkflow(workflow: Workflow) {
    if (!session) return;
    setBusy(true);

    try {
      const executionId = crypto.randomUUID();
      const historyItem: HistoryItem = {
        id: executionId,
        sessionId: session.id,
        source: workflow.source,
        userInput: workflow.originalIntent ?? workflow.label,
        executedCommand: workflow.command,
        status: "planned",
        createdAt: new Date().toISOString(),
      };
      appendHistoryItem(historyItem);
      executionToHistoryRef.current[executionId] = executionId;
      void historyAppend({ item: historyItem });

      await executeCommand({
        executionId,
        sessionId: session.id,
        command: workflow.command,
        source: workflow.source,
      });

      setWorkflowOpen(false);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  // --- Memory handlers ---
  async function handleAcceptSuggestion(suggestionId: string) {
    try {
      const res = await memoryAcceptSuggestion({ suggestionId });
      if (res.createdItem) {
        addMemoryItem(res.createdItem);
      }
      removeSuggestion(suggestionId);

      // Reload full memory for truth sync
      const memRes = await memoryList();
      setMemoryItems(memRes.items ?? []);
      setMemorySuggestions(memRes.suggestions ?? []);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function handleDismissSuggestion(suggestionId: string) {
    try {
      await memoryDismissSuggestion({ suggestionId });
      removeSuggestion(suggestionId);
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
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        onRerun={handleRerunHistoryItem}
        onReopenPlan={handleReopenPlan}
        onSaveWorkflow={handleSaveWorkflowFromHistory}
        onCopyCommand={(cmd) => { void navigator.clipboard.writeText(cmd); }}
      />

      <WorkflowDrawer
        isOpen={workflowOpen}
        workflows={workflows}
        onClose={() => {
          setWorkflowOpen(false);
          requestAnimationFrame(() => { restorePreviousZone(); composerRef.current?.focus(); });
        }}
        onRun={handleRunWorkflow}
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
    </div>
  );
}
