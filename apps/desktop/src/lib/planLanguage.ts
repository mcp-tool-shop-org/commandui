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

const CANNOT_EXPLAIN = "CommandUI cannot explain this command. Read it before you run it.";
const CANNOT_EXPLAIN_REST = "CommandUI cannot explain the rest of this command.";

const HELP_FLAGS = new Set(["--help", "-h"]);
const DELETE_FLAGS = new Set(["-rf", "-fr", "-r", "-f", "--force"]);

type StageExplanation = {
  sentence: string;
  parts: CommandPart[];
  touches: string[];
  /** A token in this stage has no real meaning here. */
  unexplained: boolean;
  /** Nothing in this stage has a real meaning. */
  unknown: boolean;
};

function shellWords(command: string): string[] {
  const words: string[] = [];
  const pattern = /"([^"]*)"|'([^']*)'|(\S+)/g;
  for (const match of command.matchAll(pattern)) {
    words.push(match[1] ?? match[2] ?? match[3] ?? "");
  }
  return words.filter((word) => word.length > 0);
}

function hasOpaqueOperator(command: string): boolean {
  let quote: string | null = null;
  for (let i = 0; i < command.length; i += 1) {
    const ch = command[i];
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
      continue;
    }
    if ((ch === "|" && command[i + 1] === "|") || (ch === "&" && command[i + 1] === "&")) return true;
  }
  return false;
}

/** Stages of a pipeline. `||` and `&&` stay one stage so they are not described as a pipe. */
function splitPipeline(command: string): string[][] {
  if (hasOpaqueOperator(command)) {
    const words = shellWords(command);
    return words.length > 0 ? [words] : [];
  }
  const stages: string[] = [];
  let current = "";
  let quote: string | null = null;
  for (const ch of command) {
    if (quote) {
      current += ch;
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
      current += ch;
      continue;
    }
    if (ch === "|") {
      stages.push(current);
      current = "";
      continue;
    }
    current += ch;
  }
  stages.push(current);
  return stages.map((stage) => shellWords(stage)).filter((words) => words.length > 0);
}

function unknownStage(): StageExplanation {
  return { sentence: "", parts: [], touches: [], unexplained: false, unknown: true };
}

function nextValue(words: string[], index: number): string | null {
  const value = words[index + 1];
  if (!value || value.startsWith("-")) return null;
  return value;
}

function explainGit(words: string[]): StageExplanation {
  const sub = words[1]?.toLowerCase();
  if (!sub || !GIT[sub]) return unknownStage();
  const parts: CommandPart[] = [
    { piece: words[0], meaning: "Git" },
    { piece: words[1], meaning: GIT[sub] },
  ];
  let unexplained = false;
  for (const word of words.slice(2)) {
    const lower = word.toLowerCase();
    if (lower === "--short" || HELP_FLAGS.has(lower)) {
      parts.push({ piece: word, meaning: FLAGS[lower] });
      continue;
    }
    unexplained = true;
  }
  let sentence = `${GIT[sub]}.`;
  if (sub === "status" && words.some((word) => word === "--short")) {
    sentence = "Shows a short list of what changed in this folder.";
  }
  return { sentence, parts, touches: [], unexplained, unknown: false };
}

function explainDelete(words: string[]): StageExplanation {
  const program = words[0].toLowerCase();
  const parts: CommandPart[] = [{ piece: words[0], meaning: PROGRAMS[program] ?? "Deletes" }];
  const touches: string[] = [];
  let unexplained = false;
  for (const word of words.slice(1)) {
    const lower = word.toLowerCase();
    if (DELETE_FLAGS.has(lower) || HELP_FLAGS.has(lower)) {
      parts.push({ piece: word, meaning: FLAGS[lower] });
      continue;
    }
    if (word.startsWith("-")) {
      unexplained = true;
      continue;
    }
    touches.push(word);
    parts.push({ piece: word, meaning: "A file or folder to delete" });
  }
  const sentence = touches.length
    ? `Deletes ${touches.length === 1 ? "one file or folder" : `${touches.length} files or folders`}.`
    : "Deletes files. The command does not name them, so the preview cannot list them.";
  return { sentence, parts, touches, unexplained, unknown: false };
}

function explainEcho(words: string[]): StageExplanation {
  const parts: CommandPart[] = [{ piece: words[0], meaning: "Prints text" }];
  for (const word of words.slice(1)) {
    parts.push({ piece: word, meaning: "Text that is printed" });
  }
  return { sentence: "Prints text.", parts, touches: [], unexplained: false, unknown: false };
}

function explainList(words: string[]): StageExplanation {
  const program = words[0].toLowerCase();
  const parts: CommandPart[] = [
    {
      piece: words[0],
      meaning: program === "dir" || program === "ls" ? "Lists the folder" : "Lists files and folders",
    },
  ];
  let filesOnly = false;
  let foldersOnly = false;
  let pattern: string | null = null;
  let recurse = false;
  let unexplained = false;
  for (let i = 1; i < words.length; i += 1) {
    const word = words[i];
    const lower = word.toLowerCase();
    if (lower === "-file") {
      filesOnly = true;
      parts.push({ piece: word, meaning: "Only files" });
      continue;
    }
    if (lower === "-directory") {
      foldersOnly = true;
      parts.push({ piece: word, meaning: "Only folders" });
      continue;
    }
    if (lower === "-recurse") {
      recurse = true;
      parts.push({ piece: word, meaning: "Includes folders inside folders" });
      continue;
    }
    if (lower === "-name") {
      parts.push({ piece: word, meaning: "Shows names only" });
      continue;
    }
    if (lower === "-hidden" || lower === "-force") {
      parts.push({ piece: word, meaning: "Includes hidden items" });
      continue;
    }
    if (lower === "-filter" || lower === "-include" || lower === "-exclude") {
      const value = nextValue(words, i);
      if (!value) {
        unexplained = true;
        continue;
      }
      i += 1;
      parts.push({
        piece: word,
        meaning: lower === "-exclude" ? "Skips names that match" : "Keeps names that match",
      });
      parts.push({ piece: value, meaning: "The name pattern" });
      if (lower === "-filter") pattern = value;
      continue;
    }
    if (lower === "-path" || lower === "-literalpath") {
      const value = nextValue(words, i);
      if (!value) {
        unexplained = true;
        continue;
      }
      i += 1;
      parts.push({ piece: word, meaning: "Names the folder to list" });
      parts.push({ piece: value, meaning: "The folder to list" });
      continue;
    }
    if (HELP_FLAGS.has(lower)) {
      parts.push({ piece: word, meaning: FLAGS[lower] });
      continue;
    }
    unexplained = true;
  }
  const specific = filesOnly || foldersOnly || pattern || recurse || program === "get-childitem" || program === "gci";
  let sentence = specific ? "Lists files and folders" : "Lists the folder";
  if (filesOnly && !foldersOnly) sentence = "Lists files";
  else if (foldersOnly && !filesOnly) sentence = "Lists folders";
  if (pattern) sentence += ` whose names match ${pattern}`;
  if (recurse) sentence += ", including folders inside folders";
  return {
    sentence: `${sentence}.`,
    parts,
    touches: [],
    unexplained,
    unknown: false,
  };
}

function explainSort(words: string[]): StageExplanation {
  const parts: CommandPart[] = [{ piece: words[0], meaning: "Sorts the results" }];
  const props: string[] = [];
  let descending = false;
  let unexplained = false;
  for (let i = 1; i < words.length; i += 1) {
    const word = words[i];
    const lower = word.toLowerCase();
    if (lower === "-descending") {
      descending = true;
      parts.push({ piece: word, meaning: "In reverse order" });
      continue;
    }
    if (lower === "-unique") {
      parts.push({ piece: word, meaning: "Drops duplicates" });
      continue;
    }
    if (lower === "-property") {
      const value = nextValue(words, i);
      if (!value) {
        unexplained = true;
        continue;
      }
      i += 1;
      props.push(value);
      parts.push({ piece: word, meaning: "Names the value to sort by" });
      parts.push({ piece: value, meaning: "The value to sort by" });
      continue;
    }
    if (HELP_FLAGS.has(lower)) {
      parts.push({ piece: word, meaning: FLAGS[lower] });
      continue;
    }
    if (word.startsWith("-")) {
      unexplained = true;
      continue;
    }
    props.push(word);
    parts.push({ piece: word, meaning: "The value to sort by" });
  }
  let sentence = "Sorts the results";
  if (props.length === 1) {
    sentence = `Sorts them by ${props[0]}`;
    if (descending) {
      sentence += props[0].toLowerCase() === "length" ? ", largest first" : ", in reverse order";
    }
  } else if (props.length > 1) {
    sentence = `Sorts them by ${props.join(" and ")}`;
    if (descending) sentence += ", in reverse order";
  } else if (descending) {
    sentence = "Sorts the results in reverse order";
  }
  if (descending && props.length === 1 && props[0].toLowerCase() === "length") {
    const flag = parts.find((part) => part.piece.toLowerCase() === "-descending");
    if (flag) flag.meaning = "Largest first";
  }
  return {
    sentence: `${sentence}.`,
    parts,
    touches: [],
    unexplained,
    unknown: false,
  };
}

function explainSelect(words: string[]): StageExplanation {
  const parts: CommandPart[] = [{ piece: words[0], meaning: "Chooses which results to keep" }];
  let first: string | null = null;
  let last: string | null = null;
  let unexplained = false;
  for (let i = 1; i < words.length; i += 1) {
    const word = words[i];
    const lower = word.toLowerCase();
    if (lower === "-first" || lower === "-last") {
      const value = nextValue(words, i);
      if (!value) {
        unexplained = true;
        continue;
      }
      i += 1;
      if (lower === "-first") first = value;
      else last = value;
      parts.push({
        piece: word,
        meaning: lower === "-first" ? "How many to keep from the start" : "How many to keep from the end",
      });
      parts.push({ piece: value, meaning: "The number to keep" });
      continue;
    }
    if (lower === "-property") {
      const value = nextValue(words, i);
      if (!value) {
        unexplained = true;
        continue;
      }
      i += 1;
      parts.push({ piece: word, meaning: "Names a field to keep" });
      parts.push({ piece: value, meaning: "A field to keep" });
      continue;
    }
    if (HELP_FLAGS.has(lower)) {
      parts.push({ piece: word, meaning: FLAGS[lower] });
      continue;
    }
    if (word.startsWith("-")) {
      unexplained = true;
      continue;
    }
    parts.push({ piece: word, meaning: "A field to keep" });
  }
  let sentence = "Chooses which results to keep.";
  if (first && !last) sentence = `Keeps the first ${first}.`;
  else if (last && !first) sentence = `Keeps the last ${last}.`;
  else if (first && last) sentence = `Keeps the first ${first} and the last ${last}.`;
  return { sentence, parts, touches: [], unexplained, unknown: false };
}

function explainCd(words: string[]): StageExplanation {
  const parts: CommandPart[] = [{ piece: words[0], meaning: "Changes the folder" }];
  let unexplained = false;
  let tookFolder = false;
  for (const word of words.slice(1)) {
    if (word.startsWith("-")) {
      if (HELP_FLAGS.has(word.toLowerCase())) {
        parts.push({ piece: word, meaning: FLAGS[word.toLowerCase()] });
      } else {
        unexplained = true;
      }
      continue;
    }
    if (!tookFolder) {
      tookFolder = true;
      parts.push({ piece: word, meaning: "The folder to use" });
      continue;
    }
    unexplained = true;
  }
  return {
    sentence: "Changes the folder.",
    parts,
    touches: [],
    unexplained,
    unknown: false,
  };
}

function explainShowFile(words: string[]): StageExplanation {
  const parts: CommandPart[] = [{ piece: words[0], meaning: "Shows a file" }];
  let unexplained = false;
  let tookFile = false;
  for (const word of words.slice(1)) {
    if (word.startsWith("-")) {
      if (HELP_FLAGS.has(word.toLowerCase())) {
        parts.push({ piece: word, meaning: FLAGS[word.toLowerCase()] });
      } else {
        unexplained = true;
      }
      continue;
    }
    if (!tookFile) {
      tookFile = true;
      parts.push({ piece: word, meaning: "The file to show" });
      continue;
    }
    unexplained = true;
  }
  return {
    sentence: "Shows a file.",
    parts,
    touches: [],
    unexplained,
    unknown: false,
  };
}

function explainKnownProgram(words: string[]): StageExplanation {
  const program = words[0].toLowerCase();
  const meaning = PROGRAMS[program] ?? "";
  if (!meaning) return unknownStage();
  const parts: CommandPart[] = [{ piece: words[0], meaning }];
  let unexplained = false;
  let tookScript = false;
  const scriptMeaning = program === "cargo" ? "The tool this runs" : "The script this runs";
  const takesScript = program === "npm" || program === "pnpm" || program === "cargo";
  for (const word of words.slice(1)) {
    const lower = word.toLowerCase();
    if (HELP_FLAGS.has(lower)) {
      parts.push({ piece: word, meaning: FLAGS[lower] });
      continue;
    }
    if (takesScript && !tookScript && !word.startsWith("-")) {
      tookScript = true;
      parts.push({ piece: word, meaning: scriptMeaning });
      continue;
    }
    unexplained = true;
  }
  return {
    sentence: `${meaning}.`,
    parts,
    touches: [],
    unexplained,
    unknown: false,
  };
}

function explainStage(words: string[]): StageExplanation {
  const program = words[0]?.toLowerCase() ?? "";
  if (program === "git") return explainGit(words);
  if (DELETE_PROGRAMS.has(program)) return explainDelete(words);
  if (program === "echo") return explainEcho(words);
  if (program === "get-childitem" || program === "gci" || program === "dir" || program === "ls") {
    return explainList(words);
  }
  if (program === "sort-object") return explainSort(words);
  if (program === "select-object") return explainSelect(words);
  if (program === "cd" || program === "chdir" || program === "set-location" || program === "sl") {
    return explainCd(words);
  }
  if (program === "cat" || program === "type" || program === "get-content" || program === "gc") {
    return explainShowFile(words);
  }
  if (PROGRAMS[program]) return explainKnownProgram(words);
  return unknownStage();
}

export function explainCommand(command: string): {
  sentence: string;
  parts: CommandPart[];
  touches: string[];
} {
  const stages = splitPipeline(command.trim());
  if (stages.length === 0) {
    return { sentence: "This plan has no command yet.", parts: [], touches: [] };
  }
  const explained = stages.map(explainStage);
  const known = explained.filter((stage) => !stage.unknown);
  const touches = explained.flatMap((stage) => stage.touches);
  if (known.length === 0) {
    return { sentence: CANNOT_EXPLAIN, parts: [], touches };
  }
  const parts: CommandPart[] = [];
  const piped = !hasOpaqueOperator(command) && stages.length > 1;
  explained.forEach((stage, index) => {
    if (piped && index > 0) {
      parts.push({ piece: "|", meaning: "Sends the results to the next step" });
    }
    parts.push(...stage.parts);
  });
  const bodies = explained
    .filter((stage) => stage.sentence)
    .map((stage) => stage.sentence.replace(/\.$/, ""))
    .map((body, index) => (index === 0 ? body : body.charAt(0).toLowerCase() + body.slice(1)));
  let sentence = explained.length === 1 ? explained[0].sentence : `${bodies.join(", then ")}.`;
  if (explained.some((stage) => stage.unexplained || stage.unknown)) {
    sentence = `${sentence} ${CANNOT_EXPLAIN_REST}`;
  }
  return { sentence, parts, touches };
}
