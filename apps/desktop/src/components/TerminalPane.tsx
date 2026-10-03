import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { useFocusStore } from "@commandui/state";
import "@xterm/xterm/css/xterm.css";

export type TerminalPaneHandle = {
  write: (data: string) => void;
  clear: () => void;
  focus: () => void;
};

type Props = {
  sessionId?: string | null;
  executionStatus?: "idle" | "running" | "success" | "failure" | "interrupted";
  onResize?: (cols: number, rows: number) => void;
  onData?: (data: string) => void;
  autoFocus?: boolean;
};

export const TerminalPane = forwardRef<TerminalPaneHandle, Props>(
  function TerminalPane(
    {
      sessionId,
      executionStatus = "idle",
      onResize,
      onData,
      autoFocus = false,
    },
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement | null>(null);
    const terminalRef = useRef<Terminal | null>(null);
    const fitRef = useRef<FitAddon | null>(null);

    const setFocusZone = useFocusStore((s) => s.setFocusZone);

    // The handlers change identity with the active session. Reading them through refs
    // keeps one xterm instance alive across tab switches instead of disposing and
    // rebuilding it (and re-parsing the whole replay) every time.
    const onDataRef = useRef(onData);
    const onResizeRef = useRef(onResize);
    onDataRef.current = onData;
    onResizeRef.current = onResize;

    // Expose imperative write/clear/focus to parent
    useImperativeHandle(
      ref,
      () => ({
        write(data: string) {
          terminalRef.current?.write(data);
        },
        clear() {
          const term = terminalRef.current;
          if (!term) return;
          term.clear();
          term.reset();
        },
        focus() {
          terminalRef.current?.focus();
        },
      }),
      [],
    );

    useEffect(() => {
      if (!containerRef.current || terminalRef.current) return;

      const term = new Terminal({
        cursorBlink: true,
        fontSize: 14,
        fontFamily:
          "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        theme: {
          background: "#171c22",
          foreground: "#e8ebf0",
        },
        scrollback: 5000,
        convertEol: true,
      });

      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(containerRef.current);
      fit.fit();

      terminalRef.current = term;
      fitRef.current = fit;

      const resizeObserver = new ResizeObserver(() => {
        fit.fit();
        onResizeRef.current?.(term.cols, term.rows);
      });

      resizeObserver.observe(containerRef.current);

      // Ctrl+Shift+<letter> is the app's escape hatch out of the terminal. xterm would
      // otherwise turn it into a control character; let it bubble to the window
      // shortcut handler instead. Copy/paste chords stay with the terminal.
      term.attachCustomKeyEventHandler((event) => {
        if (
          event.type === "keydown" &&
          event.ctrlKey &&
          event.shiftKey &&
          !event.altKey &&
          /^[a-z]$/i.test(event.key) &&
          !/^[cv]$/i.test(event.key)
        ) {
          return false;
        }
        return true;
      });

      const disposable = term.onData((data) => {
        onDataRef.current?.(data);
      });

      // Track focus zone for shortcut context
      const textarea = term.textarea;
      const handleFocus = () => setFocusZone("terminal");
      if (textarea) {
        textarea.addEventListener("focus", handleFocus);
      }

      return () => {
        disposable.dispose();
        if (textarea) {
          textarea.removeEventListener("focus", handleFocus);
        }
        resizeObserver.disconnect();
        term.dispose();
        terminalRef.current = null;
        fitRef.current = null;
      };
    }, [setFocusZone]);

    // Clear and reset on session change
    useEffect(() => {
      const term = terminalRef.current;
      if (!term) return;
      term.clear();
      term.reset();
    }, [sessionId]);

    // Cursor blink when running
    useEffect(() => {
      const term = terminalRef.current;
      if (!term) return;
      term.options.cursorBlink = executionStatus === "running";
    }, [executionStatus]);

    // Re-fit on session change
    useEffect(() => {
      const term = terminalRef.current;
      const fit = fitRef.current;
      if (!term || !fit) return;
      fit.fit();
      onResizeRef.current?.(term.cols, term.rows);
    }, [sessionId]);

    // Auto-focus
    useEffect(() => {
      if (autoFocus) {
        terminalRef.current?.focus();
      }
    }, [autoFocus, sessionId]);

    return (
      <div className="terminal-shell">
        <div className="terminal-meta">
          <span className={`exec-badge exec-${executionStatus}`}>
            {executionStatus}
          </span>
        </div>
        <div ref={containerRef} className="terminal-xterm-host" />
      </div>
    );
  },
);
