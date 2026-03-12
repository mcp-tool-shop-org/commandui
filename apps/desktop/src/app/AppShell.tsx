import { useCallback, useEffect, useRef, useState } from "react";
import type {
  CommandPlan,
  HistoryItem,
  MemorySuggestion,
  SessionSummary,
  Workflow,
} from "@commandui/domain";
import type { PlannerGeneratePlanResponse } from "@commandui/api-contract";
import {
  useComposerStore,
  useExecutionStore,
  useHistoryStore,
  useSessionStore,
  useMemoryStore,
  useSettingsStore,
  useWorkflowStore,
  resolveEffectiveMemory,
} from "@commandui/state";
import {
  createSession,
  listSessions,
  closeSession,
  executeCommand,
  resizeTerminal,
  writeTerminal,
} from "../features/terminal/terminalClient";
import {
  subscribeToTerminalLines,
  subscribeToExecutionStarted,
  subscribeToExecutionFinished,
  subscribeToSessionCwdChanged,
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
import { PlanPanel } from "../components/PlanPanel";
import { TerminalPane } from "../components/TerminalPane";
import { HistoryDrawer } from "../components/HistoryDrawer";
import { SessionTabs } from "../components/SessionTabs";
import { SettingsDrawer } from "../components/SettingsDrawer";
import { MemorySuggestions } from "../components/MemorySuggestions";
import { MemoryDrawer } from "../components/MemoryDrawer";
import { WorkflowDrawer } from "../components/WorkflowDrawer";
import { isTauriRuntime } from "../lib/tauriInvoke";

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
  } = useExecutionStore();
  const { items: historyItems, appendHistoryItem, updateHistoryItem } =
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

  const [terminalLinesBySession, setTerminalLinesBySession] = useState<
    Record<string, string[]>
  >({});
  const executionToHistoryRef = useRef<Record<string, string>>({});

  const session =
    sessions.find((s) => s.id === activeSessionId) ?? null;
  const terminalLines = activeSessionId
    ? terminalLinesBySession[activeSessionId] ?? []
    : [];

  const visibleHistoryItems = activeSessionId
    ? historyItems.filter((h) => h.sessionId === activeSessionId)
    : historyItems;

  // --- Helpers ---
  function appendTerminalLine(sessionId: string, line: string) {
    if (reducedClutter && (line.startsWith("[exec:") || line.startsWith("[active]"))) return;
    setTerminalLinesBySession((prev) => ({
      ...prev,
      [sessionId]: [...(prev[sessionId] ?? []), line],
    }));
  }

  // --- Boot / hydration ---
  useEffect(() => {
    async function boot() {
      if (browserPreview) return; // Skip Tauri calls in browser-only mode

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
            "Welcome to CommandUI. Use Command mode for explicit commands and Ask mode for semantic review.",
          );
        }

        // History
        try {
          const histRes = await historyList({ limit: 100 });
          if (histRes.items?.length) {
            for (const item of [...histRes.items].reverse()) {
              appendHistoryItem(item);
            }
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
    if (browserPreview) return;
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
      setExecutionStatus(status);

      const historyId =
        executionToHistoryRef.current[event.executionId] ?? event.executionId;
      updateHistoryItem(historyId, {
        status,
        exitCode: event.exitCode,
      });

      void historyUpdate({
        historyId,
        status,
        exitCode: event.exitCode,
      });
    }).then((u) => unlisteners.push(u));

    subscribeToSessionCwdChanged((event) => {
      updateSession(event.sessionId, { cwd: event.cwd });
    }).then((u) => unlisteners.push(u));

    return () => {
      for (const u of unlisteners) u();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  // --- Keyboard shortcuts ---
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.ctrlKey && e.key === "h") {
        e.preventDefault();
        setHistoryOpen((v) => !v);
      } else if (e.ctrlKey && e.key === "w") {
        e.preventDefault();
        setWorkflowOpen((v) => !v);
      } else if (e.ctrlKey && e.key === "m") {
        e.preventDefault();
        setMemoryOpen((v) => !v);
      } else if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        setSettingsOpen((v) => !v);
      } else if (e.ctrlKey && e.key === "1") {
        e.preventDefault();
        setInputMode("command");
      } else if (e.ctrlKey && e.key === "2") {
        e.preventDefault();
        setInputMode("ask");
      } else if (e.key === "Escape") {
        setHistoryOpen(false);
        setWorkflowOpen(false);
        setMemoryOpen(false);
        setSettingsOpen(false);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [setInputMode]);

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
        };
        appendHistoryItem(historyItem);

        void historyAppend({ item: historyItem });
        void planStore({ plan: res.plan });

        appendTerminalLine(session.id, `? ${value}`);
        appendTerminalLine(session.id, `[plan] ${res.plan.command}`);
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

      appendTerminalLine(session.id, `[approved] ${approvedCommand}`);

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

    appendTerminalLine(session.id, "[rejected]");
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
    appendTerminalLine(session.id, `[workflow:saved] ${workflow.label}`);
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

  // --- Session handlers ---
  async function handleCreateSession() {
    try {
      const label = `Session ${sessions.length + 1}`;
      const res = await createSession({ label });
      addSession(res.session);
      setActiveSessionId(res.session.id);
      appendTerminalLine(res.session.id, `[session] ${res.session.label}`);
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
    appendTerminalLine(session.id, `[workflow:saved] ${workflow.label}`);
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

          <TerminalPane
            sessionId={activeSessionId}
            lines={terminalLines}
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
            mode={inputMode}
            onModeChange={setInputMode}
            onSubmit={handleSubmit}
            busy={busy}
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
        onClose={() => setHistoryOpen(false)}
        onRerun={handleRerunHistoryItem}
        onReopenPlan={handleReopenPlan}
        onSaveWorkflow={handleSaveWorkflowFromHistory}
      />

      <WorkflowDrawer
        isOpen={workflowOpen}
        workflows={workflows}
        onClose={() => setWorkflowOpen(false)}
        onRun={handleRunWorkflow}
      />

      <MemoryDrawer
        isOpen={memoryOpen}
        items={memoryItems}
        onClose={() => setMemoryOpen(false)}
        onDelete={handleDeleteMemory}
      />

      <SettingsDrawer
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
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
