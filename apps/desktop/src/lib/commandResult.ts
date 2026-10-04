/**
 * Plain-language result of one command. The words live here so every surface
 * (the result line, history, Ask) says the same thing, and no surface has to
 * show the raw status word.
 */

export type ResultPhase = "running" | "success" | "failure" | "interrupted" | "unknown" | "request";

export type ResultAction = "show-output" | "ask-fix" | "run-again" | "ask-instead" | "stop";

export type ResultCause =
  | "ok"
  | "running"
  | "interrupted"
  | "command_not_found"
  | "access_denied"
  | "path_not_found"
  | "exit_unknown"
  | "shell_exited"
  | "input_not_accepted"
  | "request"
  | "failed";

export type CommandResult = {
  phase: ResultPhase;
  cause: ResultCause;
  headline: string;
  reason: string;
  next: string;
  actions: ResultAction[];
  /** True when this sentence should be read once by the polite status region. */
  announce: boolean;
};

export type DescribeInput = {
  phase: ResultPhase;
  exitCode?: number | null;
  /** False when the runtime invented a code or the code does not describe the outcome. */
  exitKnown?: boolean;
  outputLines?: number;
  outputText?: string;
  command?: string;
  /** Runtime reason: exit_unknown, shell_exited, input_not_accepted. */
  reason?: string | null;
};

const NOT_FOUND =
  /not recognized as an internal or external command|command not found|not recognized as (?:a|the) name of a cmdlet/i;
const ACCESS_DENIED = /access is denied|permission denied|access to the path .+ is denied/i;
const PATH_MISSING =
  /cannot find the path|no such file or directory|the system cannot find the (?:file|path)|cannot find path .+ because it does not exist/i;

const REQUEST_START =
  /^(?:please|how|what|why|when|where|who|can you|could you|would you|show me|tell me|help me|list the|i want|i need|explain|find the)\b/i;

/** How many trailing output lines Ask receives with a failed command. */
export const ASK_FIX_TAIL_LINES = 40;

const FAILURE_ACTIONS: ResultAction[] = ["show-output", "ask-fix", "run-again"];

/**
 * A Command-mode line that reads like a sentence rather than a command.
 * A single token, a path, a flag, or a shell metacharacter stays a command.
 */
export function looksLikeRequest(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed) return false;
  if (trimmed.includes("?")) return true;
  if (/[\\/|&;<>`$(){}]/.test(trimmed)) return false;
  if (/(?:^|\s)-{1,2}\S/.test(trimmed)) return false;
  if (!trimmed.includes(" ")) return false;
  if (REQUEST_START.test(trimmed)) return true;
  const words = trimmed.split(/\s+/);
  return words.length >= 4 && words.every((word) => /^[A-Za-z0-9'.,-]+$/.test(word));
}

const MAX_SCREEN_ROWS = 5000;

/**
 * What the output view should read. ConPTY repaints with cursor moves and
 * colour codes; a carriage-return split keeps only the last piece of a line
 * and prints those codes. This applies the codes and keeps every row that
 * was not overwritten.
 */
export function collapseRedraws(text: string, command?: string): string {
  return stripChrome(renderScreen(text), command).join("\n");
}

export function countOutputLines(text: string, command?: string): number {
  return collapseRedraws(text, command)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean).length;
}

function stripChrome(lines: string[], command?: string): string[] {
  const echo = command?.trim() ?? "";
  let droppedEcho = false;
  const kept = lines.filter((line) => {
    const trimmed = line.trim();
    // The shell prompt is `>`. A line that is only that prompt is not output.
    if (trimmed === ">") return false;
    if (!droppedEcho && echo && (trimmed === `> ${echo}` || trimmed === `>${echo}`)) {
      droppedEcho = true;
      return false;
    }
    return true;
  });
  while (kept.length > 0 && kept[kept.length - 1] === "") kept.pop();
  return kept;
}

function renderScreen(text: string): string[] {
  const rows: string[][] = [[]];
  let row = 0;
  let col = 0;
  let saved: { row: number; col: number } | null = null;

  const ensure = (index: number) => {
    while (rows.length <= index && rows.length < MAX_SCREEN_ROWS) rows.push([]);
  };
  const put = (ch: string) => {
    ensure(row);
    const line = rows[row];
    while (line.length < col) line.push(" ");
    if (col < line.length) line[col] = ch;
    else line.push(ch);
    col += 1;
  };
  const eraseLine = (mode: number) => {
    ensure(row);
    const line = rows[row];
    if (mode === 0) line.splice(col);
    else if (mode === 1) {
      for (let at = 0; at <= col && at < line.length; at += 1) line[at] = " ";
    } else line.splice(0, line.length);
  };
  const eraseDisplay = (mode: number) => {
    if (mode === 0) {
      eraseLine(0);
      rows.splice(row + 1);
    } else if (mode === 1) {
      for (let i = 0; i < row; i += 1) rows[i] = [];
      eraseLine(1);
    } else {
      rows.splice(0, rows.length, []);
      row = 0;
      col = 0;
    }
  };

  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (ch === "\u001b") {
      const next = text[i + 1];
      if (next === "[") {
        const csi = readCsi(text, i + 1);
        if (!csi) continue;
        i = csi.next - 1;
        applyCsi(csi, {
          privateMark: csi.privateMark,
          final: csi.final,
          params: csi.params,
          move(nextRow: number, nextCol: number) {
            row = Math.max(0, Math.min(MAX_SCREEN_ROWS - 1, nextRow));
            col = Math.max(0, nextCol);
            ensure(row);
          },
          eraseLine,
          eraseDisplay,
          deleteChars(count: number) {
            ensure(row);
            rows[row].splice(col, count);
          },
          eraseChars(count: number) {
            ensure(row);
            const line = rows[row];
            for (let at = col; at < col + count && at < line.length; at += 1) line[at] = " ";
          },
          save() {
            saved = { row, col };
          },
          restore() {
            if (!saved) return;
            row = saved.row;
            col = saved.col;
            ensure(row);
          },
          row: () => row,
          col: () => col,
        });
        continue;
      }
      if (next === "]") {
        const end = readOsc(text, i + 1);
        if (end < 0) break;
        i = end - 1;
        continue;
      }
      if (next) i += 1;
      continue;
    }
    if (ch === "\r") {
      col = 0;
      continue;
    }
    if (ch === "\n") {
      row += 1;
      col = 0;
      if (row >= MAX_SCREEN_ROWS) {
        rows.shift();
        row = MAX_SCREEN_ROWS - 1;
      }
      ensure(row);
      continue;
    }
    if (ch === "\u0007" || ch === "\u0000") continue;
    if (ch === "\u0008") {
      col = Math.max(0, col - 1);
      continue;
    }
    put(ch);
  }

  return rows.map((line) => line.join("").replace(/[ \t]+$/g, ""));
}

type Csi = { next: number; params: number[]; privateMark: boolean; final: string };

function readCsi(text: string, bracket: number): Csi | null {
  let j = bracket + 1;
  const start = j;
  while (j < text.length) {
    const code = text.charCodeAt(j);
    if (code < 0x30 || code > 0x3f) break;
    j += 1;
  }
  const paramStr = text.slice(start, j);
  while (j < text.length) {
    const code = text.charCodeAt(j);
    if (code < 0x20 || code > 0x2f) break;
    j += 1;
  }
  if (j >= text.length) return null;
  const final = text[j];
  const finalCode = final.charCodeAt(0);
  if (finalCode < 0x40 || finalCode > 0x7e) return null;
  const params = paramStr
    .replace(/[^0-9;]/g, "")
    .split(";")
    .map((part) => (part.length === 0 ? 0 : Number.parseInt(part, 10)));
  if (paramStr.length === 0) params.length = 0;
  return { next: j + 1, params, privateMark: paramStr.includes("?"), final };
}

function readOsc(text: string, bracket: number): number {
  let j = bracket + 1;
  while (j < text.length) {
    if (text[j] === "\u0007") return j + 1;
    if (text[j] === "\u001b" && text[j + 1] === "\\") return j + 2;
    j += 1;
  }
  return -1;
}

function csiCount(params: number[], index: number, fallback: number): number {
  const value = params[index];
  if (value == null || value === 0) return fallback;
  return value;
}

function applyCsi(
  csi: Csi,
  screen: {
    privateMark: boolean;
    final: string;
    params: number[];
    move: (row: number, col: number) => void;
    eraseLine: (mode: number) => void;
    eraseDisplay: (mode: number) => void;
    deleteChars: (count: number) => void;
    eraseChars: (count: number) => void;
    save: () => void;
    restore: () => void;
    row: () => number;
    col: () => number;
  },
): void {
  const { final, params, privateMark } = csi;
  if (privateMark && (final === "h" || final === "l")) return;
  if (final === "m") return;
  if (final === "H" || final === "f") {
    screen.move(csiCount(params, 0, 1) - 1, csiCount(params, 1, 1) - 1);
    return;
  }
  if (final === "A") {
    screen.move(screen.row() - csiCount(params, 0, 1), screen.col());
    return;
  }
  if (final === "B") {
    screen.move(screen.row() + csiCount(params, 0, 1), screen.col());
    return;
  }
  if (final === "C") {
    screen.move(screen.row(), screen.col() + csiCount(params, 0, 1));
    return;
  }
  if (final === "D") {
    screen.move(screen.row(), screen.col() - csiCount(params, 0, 1));
    return;
  }
  if (final === "G") {
    screen.move(screen.row(), csiCount(params, 0, 1) - 1);
    return;
  }
  if (final === "J") {
    screen.eraseDisplay(csiCount(params, 0, 0));
    return;
  }
  if (final === "K") {
    screen.eraseLine(params[0] ?? 0);
    return;
  }
  if (final === "P") {
    screen.deleteChars(csiCount(params, 0, 1));
    return;
  }
  if (final === "X") {
    screen.eraseChars(csiCount(params, 0, 1));
    return;
  }
  if (final === "s") screen.save();
  else if (final === "u") screen.restore();
}

export function resultText(result: CommandResult): string {
  return [result.headline, result.reason, result.next].filter(Boolean).join(" ");
}

function withExit(code: number | null | undefined, known: boolean): string {
  if (!known || code == null) return "Did not work";
  return `Did not work (exit code ${code})`;
}

function finishedLines(count: number): string {
  const lines = count === 1 ? "1 line" : `${count} lines`;
  return `Finished. ${lines} of output.`;
}

export function describeResult(input: DescribeInput): CommandResult {
  if (input.phase === "running") {
    return {
      phase: "running",
      cause: "running",
      headline: "Running… (Stop)",
      reason: "",
      next: "",
      actions: ["stop"],
      announce: false,
    };
  }

  if (input.phase === "request") {
    return {
      phase: "request",
      cause: "request",
      headline: "This looks like a request. Ask CommandUI instead?",
      reason: "That reads like a question, so it was not run as a command.",
      next: "Ask CommandUI, or edit it into a command and run it.",
      actions: ["ask-instead"],
      announce: false,
    };
  }

  if (input.phase === "interrupted") {
    return {
      phase: "interrupted",
      cause: "interrupted",
      headline: "Stopped.",
      reason: "The command was stopped before it finished.",
      next: "Run it again if you still want it.",
      actions: ["run-again"],
      announce: true,
    };
  }

  if (input.phase === "success") {
    const count = input.outputLines ?? countOutputLines(input.outputText ?? "", input.command);
    return {
      phase: "success",
      cause: "ok",
      headline: finishedLines(count),
      reason: "",
      next: "",
      actions: count > 0 ? ["show-output"] : [],
      announce: true,
    };
  }

  if (input.reason === "shell_exited") {
    return {
      phase: "failure",
      cause: "shell_exited",
      headline: "The shell exited.",
      reason: "This session no longer has a shell.",
      next: "Open a new session to continue.",
      actions: [],
      announce: true,
    };
  }

  if (input.reason === "input_not_accepted") {
    return {
      phase: "failure",
      cause: "input_not_accepted",
      headline: "Did not work",
      reason: "The terminal did not accept that input.",
      next: "Try again. If the terminal is stuck, open a new session.",
      actions: ["run-again"],
      announce: true,
    };
  }

  const known =
    input.exitKnown !== false && input.phase !== "unknown" && input.reason !== "exit_unknown";
  const text = collapseRedraws(input.outputText ?? "");
  const code = input.exitCode ?? null;
  const outputCause: ResultCause | null =
    code === 127 || NOT_FOUND.test(text)
      ? "command_not_found"
      : code === 5 || ACCESS_DENIED.test(text)
        ? "access_denied"
        : PATH_MISSING.test(text)
          ? "path_not_found"
          : null;

  if (outputCause === "command_not_found") {
    return {
      phase: known ? "failure" : "unknown",
      cause: "command_not_found",
      headline: withExit(code, known),
      reason: "The shell could not find that command.",
      next: "Check the spelling, or ask how to fix it.",
      actions: FAILURE_ACTIONS,
      announce: true,
    };
  }

  if (outputCause === "access_denied") {
    return {
      phase: known ? "failure" : "unknown",
      cause: "access_denied",
      headline: withExit(code, known),
      reason: "CommandUI was not allowed to do that.",
      next: "Ask how to fix it, or run it again if you expected this to be allowed.",
      actions: FAILURE_ACTIONS,
      announce: true,
    };
  }

  if (outputCause === "path_not_found") {
    return {
      phase: known ? "failure" : "unknown",
      cause: "path_not_found",
      headline: withExit(code, known),
      reason: "A file or folder in that command is not there.",
      next: "Check the path, or ask how to fix it.",
      actions: FAILURE_ACTIONS,
      announce: true,
    };
  }

  if (!known) {
    return {
      phase: "unknown",
      cause: "exit_unknown",
      headline: "CommandUI could not tell whether this worked",
      reason: "The shell did not report an exit code.",
      next: "Show the output, or run the command again.",
      actions: FAILURE_ACTIONS,
      announce: true,
    };
  }

  return {
    phase: "failure",
    cause: "failed",
    headline: withExit(code ?? 1, true),
    reason: "The command finished with an error.",
    next: "Show the output, ask how to fix it, or run it again.",
    actions: FAILURE_ACTIONS,
    announce: true,
  };
}

export function askFixPrompt(input: {
  command: string;
  exitCode?: number | null;
  exitKnown?: boolean;
  outputText?: string;
  cause?: ResultCause;
}): string {
  const collapsed = collapseRedraws(input.outputText ?? "");
  const tail = collapsed.split("\n").slice(-ASK_FIX_TAIL_LINES).join("\n").trim();
  const hideCode =
    input.exitKnown === false ||
    input.cause === "exit_unknown" ||
    input.cause === "shell_exited" ||
    input.cause === "input_not_accepted" ||
    input.exitCode == null;
  const codeLine = hideCode ? "Exit code: unknown" : `Exit code: ${input.exitCode}`;
  return [
    "This command did not work. Help me fix it.",
    "",
    `Command: ${input.command || "(unknown)"}`,
    codeLine,
    "",
    "Last output:",
    tail || "(no output)",
  ].join("\n");
}

/** History chip text. Never the raw status word. */
export function historyStatusLabel(status: string): string {
  switch (status) {
    case "success":
      return "Finished";
    case "failure":
      return "Did not work";
    case "interrupted":
      return "Stopped";
    case "unknown":
      return "Could not tell";
    case "planned":
      return "Not run yet";
    case "rejected":
      return "Rejected";
    default:
      return "Could not tell";
  }
}
