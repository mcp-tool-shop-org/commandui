/** Plain words for a plan. The planner's own codes stay out of the screen. */

export const SAFETY_FLAGS = [
  "DESTRUCTIVE_OPERATION",
  "PRIVILEGE_ESCALATION",
  "HIGH_RISK_COMMAND",
  "NETWORK_ACCESS",
] as const;

const FLAG_TEXT: Record<(typeof SAFETY_FLAGS)[number], string> = {
  DESTRUCTIVE_OPERATION: "Deletes files: cannot be undone",
  PRIVILEGE_ESCALATION: "Runs with higher permissions",
  HIGH_RISK_COMMAND: "Can make a change that is hard to undo",
  NETWORK_ACCESS: "Uses the network",
};

export function flagInWords(code: string): string {
  if (Object.prototype.hasOwnProperty.call(FLAG_TEXT, code)) {
    return FLAG_TEXT[code as keyof typeof FLAG_TEXT];
  }
  return "This needs a careful look before it runs";
}

export function riskInWords(
  risk: "low" | "medium" | "high",
  destructive: boolean,
  escalates: boolean,
): string {
  if (destructive) return "Deletes files: cannot be undone";
  if (escalates) return "Runs with higher permissions";
  if (risk === "high") return "High risk. This can be hard to undo.";
  if (risk === "medium") return "Medium risk. Read the command before you run it.";
  return "Low risk. Easy to undo.";
}

/** The folder the command runs in. The user types this to approve a high-risk plan. */
export function confirmationPhrase(cwd: string | undefined): string {
  if (!cwd) return "confirm";
  const trimmed = cwd.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/).filter((part) => part.length > 0 && part !== "." && part !== "..");
  const last = parts[parts.length - 1];
  if (!last || last.endsWith(":")) return "confirm";
  return last;
}

export function confirmationMatches(typed: string, phrase: string): boolean {
  return typed.trim().toLocaleLowerCase() === phrase.toLocaleLowerCase();
}

export type CommandPart = { piece: string; meaning: string };

const PROGRAMS: Record<string, string> = {
  git: "Git",
  echo: "Prints text",
  dir: "Lists the folder",
  ls: "Lists the folder",
  rm: "Deletes",
  del: "Deletes",
  erase: "Deletes",
  rmdir: "Deletes a folder",
  "remove-item": "Deletes",
  npm: "Runs a project script",
  pnpm: "Runs a project script",
  cargo: "Runs a Rust tool",
  cat: "Shows a file",
  type: "Shows a file",
};

const GIT: Record<string, string> = {
  status: "Shows what changed",
  diff: "Shows the edits",
  log: "Shows older commits",
  push: "Sends commits to another copy",
  pull: "Brings in commits",
  add: "Stages files",
  commit: "Saves a commit",
  checkout: "Switches or restores",
  switch: "Switches branch",
  clone: "Copies a repository",
  fetch: "Downloads updates",
};

const FLAGS: Record<string, string> = {
  "--short": "Uses a short list",
  "-h": "Asks for help",
  "--help": "Asks for help",
  "-r": "Includes folders inside folders",
  "-rf": "Deletes folders and what is inside them",
  "-fr": "Deletes folders and what is inside them",
  "-f": "Does not ask first",
  "--force": "Does not ask first",
};

const DELETE_PROGRAMS = new Set(["rm", "del", "erase", "rmdir", "unlink", "remove-item"]);

function shellWords(command: string): string[] {
  const words: string[] = [];
  const pattern = /"([^"]*)"|'([^']*)'|(\S+)/g;
  for (const match of command.matchAll(pattern)) {
    words.push(match[1] ?? match[2] ?? match[3] ?? "");
  }
  return words.filter((word) => word.length > 0);
}

export function explainCommand(command: string): {
  sentence: string;
  parts: CommandPart[];
  touches: string[];
} {
  const words = shellWords(command.trim());
  if (words.length === 0) {
    return { sentence: "This plan has no command yet.", parts: [], touches: [] };
  }
  const program = words[0].toLowerCase();
  const programMeaning = PROGRAMS[program] ?? `Runs ${words[0]}`;
  const parts: CommandPart[] = [{ piece: words[0], meaning: programMeaning }];
  const touches: string[] = [];
  const deleting = DELETE_PROGRAMS.has(program);
  for (const word of words.slice(1)) {
    const lower = word.toLowerCase();
    if (program === "git" && GIT[lower]) {
      parts.push({ piece: word, meaning: GIT[lower] });
      continue;
    }
    if (FLAGS[lower] || word.startsWith("-")) {
      parts.push({ piece: word, meaning: FLAGS[lower] ?? "Changes how the command runs" });
      continue;
    }
    if (deleting) touches.push(word);
    parts.push({
      piece: word,
      meaning: deleting ? "A file or folder to delete" : "Passed to the program",
    });
  }

  let sentence = `${programMeaning}.`;
  if (program === "git" && words[1] && GIT[words[1].toLowerCase()]) {
    sentence = `${GIT[words[1].toLowerCase()]}.`;
    if (words[1].toLowerCase() === "status" && words.some((word) => word === "--short")) {
      sentence = "Shows a short list of what changed in this folder.";
    }
  } else if (deleting) {
    sentence = touches.length
      ? `Deletes ${touches.length === 1 ? "one file or folder" : `${touches.length} files or folders`}.`
      : "Deletes files. The command does not name them, so the preview cannot list them.";
  } else if (program === "echo") {
    sentence = "Prints text.";
  }
  return { sentence, parts, touches };
}
