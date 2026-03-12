import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

type Props = {
  sessionId?: string | null;
  lines: string[];
  executionStatus?: "idle" | "running" | "success" | "failure";
  onResize?: (cols: number, rows: number) => void;
  onData?: (data: string) => void;
  autoFocus?: boolean;
};

export function TerminalPane({
  sessionId,
  lines,
  executionStatus = "idle",
  onResize,
  onData,
  autoFocus = false,
}: Props) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const lastRenderedLineCountRef = useRef(0);

  useEffect(() => {
    if (!containerRef.current || terminalRef.current) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
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
    lastRenderedLineCountRef.current = 0;
  }, [sessionId]);

  // Render new lines incrementally
  useEffect(() => {
    const term = terminalRef.current;
    if (!term) return;

    if (lastRenderedLineCountRef.current === 0 && sessionId) {
      term.writeln(`[session] ${sessionId}`);
    }

    const nextLines = lines.slice(lastRenderedLineCountRef.current);
    for (const line of nextLines) {
      term.write(line);
      if (!line.endsWith("\n") && !line.endsWith("\r\n")) {
        term.write("\r\n");
      }
    }

    lastRenderedLineCountRef.current = lines.length;
  }, [lines, sessionId]);

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
}
