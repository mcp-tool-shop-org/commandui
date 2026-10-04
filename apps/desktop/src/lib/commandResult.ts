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

const NOT_FOUND = /not recognized as an internal or external command|command not found/i;
const ACCESS_DENIED = /access is denied|permission denied/i;
const PATH_MISSING = /cannot find the path|no such file or directory|the system cannot find the (?:file|path)/i;

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

/** Progress redraws keep the last segment of each line. */
export function collapseRedraws(text: string): string {
  return text
    .split("\n")
    .map((line) => {
      const parts = line.split("\r");
      return parts[parts.length - 1] ?? "";
    })
    .join("\n");
}

export function countOutputLines(text: string): number {
  return collapseRedraws(text)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean).length;
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
    const count = input.outputLines ?? countOutputLines(input.outputText ?? "");
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
  const text = input.outputText ?? "";
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
