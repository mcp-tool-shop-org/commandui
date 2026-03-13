import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

export type TerminalPaneHandle = {
  write: (data: string) => void;
  clear: () => void;
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

    // Expose imperative write/clear to parent
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
        onResize?.(term.cols, term.rows);
      });

      resizeObserver.observe(containerRef.current);

      const disposable = term.onData((data) => {
        onData?.(data);
      });

      return () => {
        disposable.dispose();
        resizeObserver.disconnect();
        term.dispose();
        terminalRef.current = null;
        fitRef.current = null;
      };
    }, [onData, onResize]);

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
      onResize?.(term.cols, term.rows);
    }, [sessionId, onResize]);

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
