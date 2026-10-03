/**
 * Display-versus-executed safety helpers.
 *
 * A command shown in a textarea or written to the terminal must be the command
 * that runs. Unicode direction controls and zero-width characters make text render
 * differently from its bytes, and ASCII control bytes (ESC, CR, ...) make the
 * terminal redraw over what the app printed. These helpers find such characters
 * and render them as visible markers.
 */

export type HiddenChar = {
  char: string;
  codePoint: number;
  /** Human-readable class, e.g. "bidirectional control". */
  kind: string;
};

const BIDI = "bidirectional control";
const INVISIBLE = "zero-width or invisible";
const SEPARATOR = "line or paragraph separator";

const FORMAT_CHAR = /^\p{Cf}$/u;

function classify(cp: number): string | null {
  if (cp >= 0x202a && cp <= 0x202e) return BIDI;
  if (cp >= 0x2066 && cp <= 0x2069) return BIDI;
  if (cp === 0x200e || cp === 0x200f || cp === 0x061c) return BIDI;
  if (cp >= 0x200b && cp <= 0x200d) return INVISIBLE;
  if (cp >= 0x2060 && cp <= 0x2064) return INVISIBLE;
  if (cp >= 0x206a && cp <= 0x206f) return INVISIBLE;
  if (cp === 0xfeff || cp === 0x00ad || cp === 0x180e) return INVISIBLE;
  if (cp === 0x2028 || cp === 0x2029) return SEPARATOR;
  // Characters that render as nothing but are still passed to the shell: tag characters,
  // variation selectors, the grapheme joiner, Hangul and halfwidth fillers, the braille
  // blank, and interlinear annotation marks.
  if (cp >= 0xe0000 && cp <= 0xe007f) return INVISIBLE;
  if (cp >= 0xfe00 && cp <= 0xfe0f) return INVISIBLE;
  if (cp >= 0xe0100 && cp <= 0xe01ef) return INVISIBLE;
  if (cp === 0x034f || cp === 0x115f || cp === 0x1160 || cp === 0x3164 || cp === 0xffa0) return INVISIBLE;
  if (cp === 0x2800) return INVISIBLE;
  if (cp >= 0xfff9 && cp <= 0xfffb) return INVISIBLE;
  // Any other format character (Unicode category Cf) draws nothing either.
  if (FORMAT_CHAR.test(String.fromCodePoint(cp))) return INVISIBLE;
  return null;
}

export function formatCodePoint(cp: number): string {
  return `<U+${cp.toString(16).toUpperCase().padStart(4, "0")}>`;
}

/** Every hidden or direction-changing character in the text, in order. */
export function findHiddenChars(text: string): HiddenChar[] {
  const found: HiddenChar[] = [];
  for (const char of text) {
    const cp = char.codePointAt(0) as number;
    const kind = classify(cp);
    if (kind) found.push({ char, codePoint: cp, kind });
  }
  return found;
}

export function hasHiddenChars(text: string): boolean {
  return findHiddenChars(text).length > 0;
}

/** The text with every hidden character removed. */
export function stripHiddenChars(text: string): string {
  let out = "";
  for (const char of text) {
    if (classify(char.codePointAt(0) as number) === null) out += char;
  }
  return out;
}

/** The text with every hidden character shown as `<U+202E>`; everything else unchanged. */
export function markHiddenChars(text: string): string {
  let out = "";
  for (const char of text) {
    const cp = char.codePointAt(0) as number;
    out += classify(cp) ? formatCodePoint(cp) : char;
  }
  return out;
}

/** One sentence naming the classes present, or null when the text is clean. */
export function describeHiddenChars(text: string): string | null {
  const found = findHiddenChars(text);
  if (found.length === 0) return null;
  const kinds = Array.from(new Set(found.map((f) => f.kind)));
  const codes = Array.from(new Set(found.map((f) => formatCodePoint(f.codePoint))));
  return `${found.length} hidden character${found.length === 1 ? "" : "s"} (${kinds.join(", ")}): ${codes.join(" ")}`;
}

/**
 * Text for a line the app writes to the terminal itself: control characters are
 * shown as ^[ / \n style markers and hidden characters as <U+XXXX>, so a model- or
 * user-supplied string cannot move the cursor or recolour the transcript.
 */
export function escapeForTerminal(text: string): string {
  let out = "";
  for (const char of text) {
    const cp = char.codePointAt(0) as number;
    if (cp === 0x0a) out += "\\n";
    else if (cp === 0x0d) out += "\\r";
    else if (cp === 0x09) out += "\\t";
    else if (cp < 0x20) out += `^${String.fromCharCode(cp + 0x40)}`;
    else if (cp === 0x7f) out += "^?";
    else if (cp >= 0x80 && cp <= 0x9f) out += `\\x${cp.toString(16)}`;
    else if (classify(cp)) out += formatCodePoint(cp);
    else out += char;
  }
  return out;
}

/** True for ASCII control characters (including newline, CR and TAB) and DEL. */
function isAsciiControl(cp: number): boolean {
  return cp < 0x20 || cp === 0x7f;
}

/**
 * Why a command cannot be sent to the shell as typed, or null when it can. The shell
 * runs one line; a newline or control byte would be rejected by the backend, and a
 * hidden character would run text that is not what the user saw.
 */
export function commandProblem(command: string): string | null {
  for (const char of command) {
    const cp = char.codePointAt(0) as number;
    if (cp === 0x0a || cp === 0x0d) {
      return "This command has more than one line. The terminal runs one line at a time, so split it into separate commands.";
    }
    if (isAsciiControl(cp)) {
      return "This command contains a control character (such as a tab or escape), which the terminal cannot run as typed. Remove it and try again.";
    }
  }
  const hidden = describeHiddenChars(command);
  if (hidden) {
    return `This command contains ${hidden}. They change how text displays without changing what runs, so it was not sent. Remove them and try again.`;
  }
  return null;
}
