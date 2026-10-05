import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { useFocusStore } from "@commandui/state";
import "@xterm/xterm/css/xterm.css";
import { terminalTheme } from "../lib/terminalTheme";

function mediaMatches(query: string): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia(query).matches;
}

function useMedia(query: string): boolean {
  const [matches, setMatches] = useState(() => mediaMatches(query));
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const list = window.matchMedia(query);
    const onChange = () => setMatches(list.matches);
    onChange();
    list.addEventListener?.("change", onChange);
    return () => list.removeEventListener?.("change", onChange);
  }, [query]);
  return matches;
}

/**
 * xterm answers the queries inside a replayed stream (cursor position, device attributes,
 * mode and colour reports) through onData. Those replies, recognised by their shape, must
 * not reach the live shell. Nothing a user types looks like one: arrow keys, Alt chords and
 * pastes are never a whole CSI ... R / c / y / n / t report or an OSC/DCS string.
 */
const AUTO_REPLY =
  // eslint-disable-next-line no-control-regex
  /^(?:\x1b\[\??\d+;\d+R|\x1b\[[?>=]?[\d;]*c|\x1b\[\??\d+;\d+\$y|\x1b\[[0-3]n|\x1b\[\?\d+(?:;\d+)*n|\x1b\[\d+(?:;\d+)+t|\x1b\][\s\S]*(?:\x07|\x1b\\)|\x1bP[\s\S]*\x1b\\)$/;

export function isTerminalAutoReply(data: string): boolean {
  return AUTO_REPLY.test(data);
}

export type TerminalPaneHandle = {
  write: (data: string) => void;
  /**
   * Clear the terminal and write a stored stream. Only xterm's own replies to queries in the
   * stream are muted; keystrokes still reach the shell. A newer replay (or clear) cancels an
   * older one, and live output that arrives meanwhile is held until the replay has parsed.
   */
  replay: (chunks: readonly string[]) => void;
  clear: () => void;
  focus: () => void;
};

type Props = {
  sessionId?: string | null;
  /** Drives the cursor blink while a command runs. The result line lives outside this pane. */
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
    // True while a stored stream is replayed: xterm answers the queries inside it (cursor
    // position, device attributes) through onData, and those replies must not reach the shell.
    const replayingRef = useRef(false);
    // Bumped by every replay, clear and session change: a replay whose number is stale stops.
    const replayGenRef = useRef(0);
    // Live output that arrives while a replay is still parsing; written after it.
    const pendingLiveRef = useRef<string[]>([]);

    const setFocusZone = useFocusStore((s) => s.setFocusZone);
    const reduceMotion = useMedia("(prefers-reduced-motion: reduce)");
    const lightTheme = useMedia("(prefers-color-scheme: light)");
    const reduceMotionRef = useRef(reduceMotion);
    const lightThemeRef = useRef(lightTheme);
    reduceMotionRef.current = reduceMotion;
    lightThemeRef.current = lightTheme;

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
          if (replayingRef.current) {
            pendingLiveRef.current.push(data);
            return;
          }
          terminalRef.current?.write(data);
        },
        replay(chunks: readonly string[]) {
          const term = terminalRef.current;
          if (!term) return;
          const gen = ++replayGenRef.current;
          const stream = chunks.slice();
          pendingLiveRef.current = [];
          replayingRef.current = true;
          const finish = () => {
            if (gen !== replayGenRef.current) return;
            replayingRef.current = false;
            const held = pendingLiveRef.current;
            pendingLiveRef.current = [];
            for (const data of held) term.write(data);
          };
          const writeFrom = (i: number) => {
            if (gen !== replayGenRef.current) return;
            if (i >= stream.length) {
              finish();
              return;
            }
            term.write(stream[i], () => writeFrom(i + 1));
          };
          // The empty write is a barrier: its callback runs after every write an older
          // replay already queued, so none of that output can land after the reset.
          term.write("", () => {
            if (gen !== replayGenRef.current) return;
            term.clear();
            term.reset();
            writeFrom(0);
          });
        },
        clear() {
          const term = terminalRef.current;
          if (!term) return;
          replayGenRef.current++;
          replayingRef.current = false;
          pendingLiveRef.current = [];
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
        cursorBlink: !reduceMotionRef.current,
        screenReaderMode: true,
        // The shell's zoom is the text scale. Multiplying this cell size would draw the glyphs twice.
        fontSize: 14,
        fontFamily:
          "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        theme: terminalTheme(lightThemeRef.current),
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
        if (replayingRef.current && isTerminalAutoReply(data)) return;
        onDataRef.current?.(data);
      });

      // convertEol turns a bare LF into CR LF, which the main screen needs (the runtime emits
      // bare LF there) but a full-screen app does not: it uses LF to step down a row and keep the
      // column. Follow the active buffer.
      // Optional chaining: test doubles for xterm may not model buffers.
      const bufferDisposable = term.buffer?.onBufferChange?.((buffer) => {
        term.options.convertEol = buffer.type !== "alternate";
      });

      // Track focus zone for shortcut context
      const textarea = term.textarea;
      const handleFocus = () => setFocusZone("terminal");
      if (textarea) {
        textarea.addEventListener("focus", handleFocus);
      }

      return () => {
        disposable.dispose();
        bufferDisposable?.dispose();
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
      replayGenRef.current++;
      replayingRef.current = false;
      pendingLiveRef.current = [];
      term.clear();
      term.reset();
    }, [sessionId]);

    // Cursor blink only while a command runs, and never when the person asked for less motion.
    // The theme follows the light or dark setting. The cell size stays 14; zoom scales it.
    useEffect(() => {
      const term = terminalRef.current;
      if (!term) return;
      term.options.cursorBlink = !reduceMotion && executionStatus === "running";
      term.options.fontSize = 14;
      term.options.theme = {
        ...term.options.theme,
        ...terminalTheme(lightTheme),
      };
    }, [executionStatus, reduceMotion, lightTheme]);

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
        <div ref={containerRef} className="terminal-xterm-host" />
      </div>
    );
  },
);
