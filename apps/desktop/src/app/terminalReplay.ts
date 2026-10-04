/** Per-session replay buffer cap, in characters (not entries: a PTY read can be 4 KB). */
export const TERMINAL_REPLAY_MAX_CHARS = 600_000;
/** Chunks are merged up to this size so a flood of tiny reads stays a few entries. */
export const TERMINAL_REPLAY_CHUNK_CHARS = 16_000;
/** SGR reset, then a marker: truncation can cut mid-escape and mid-line, so start clean. */
export const TERMINAL_REPLAY_TRUNCATED_MARKER = "\x1b[0m[earlier output truncated]\r\n";
/**
 * Truncation of a session that is inside a full-screen app must not start the replay
 * on the main screen. Replay re-enters the alternate screen first.
 */
export const TERMINAL_REPLAY_ALT_PREFIX = "\x1b[0m\x1b[?1049h";
const ALT_SCREEN_SEQ = /\x1b\[\?(?:1049|1047|47)([hl])/g;
export const ALT_SCREEN_TAIL_CHARS = 12;
/**
 * Private modes the shell sets that a truncated or cleared replay would lose, and
 * xterm's reset would turn off. The alternate screen has its own tracking.
 */
const TRACKED_PRIVATE_MODES: ReadonlySet<number> = new Set([
  1, 9, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1015, 1016, 2004,
]);
const PRIVATE_MODE_SEQ = /\x1b\[\?([\d;]+)([hl])/g;
export const PRIVATE_MODE_TAIL_CHARS = 32;

export type ReplaySession = {
  buffer: string[];
  chars: number;
  altScreen: boolean;
  altTail: string;
  privateModes: Record<number, boolean>;
  privateModeTail: string;
};

export function emptyReplaySession(): ReplaySession {
  return {
    buffer: [],
    chars: 0,
    altScreen: false,
    altTail: "",
    privateModes: {},
    privateModeTail: "",
  };
}

/** Fold the DECSET/DECRST sequences in `text` into the last-known value per tracked mode. */
export function privateModesAfter(
  previous: Record<number, boolean>,
  text: string,
): Record<number, boolean> {
  let next = previous;
  for (const match of text.matchAll(PRIVATE_MODE_SEQ)) {
    for (const part of match[1].split(";")) {
      const mode = Number(part);
      if (!TRACKED_PRIVATE_MODES.has(mode)) continue;
      if (next === previous) next = { ...previous };
      next[mode] = match[2] === "h";
    }
  }
  return next;
}

/** The sequences that put a freshly reset xterm back into the modes the shell last set. */
export function privateModePrefix(modes: Record<number, boolean> | undefined): string {
  if (!modes) return "";
  return Object.entries(modes)
    .map(([mode, on]) => `\x1b[?${mode}${on ? "h" : "l"}`)
    .join("");
}

export function isReplayPrefix(entry: string): boolean {
  return entry === TERMINAL_REPLAY_TRUNCATED_MARKER || entry === TERMINAL_REPLAY_ALT_PREFIX;
}

export function altScreenAfter(previous: boolean, text: string): boolean {
  let state = previous;
  for (const match of text.matchAll(ALT_SCREEN_SEQ)) state = match[1] === "h";
  return state;
}

/**
 * Keep one session's replay buffer. Chunks coalesce, the alternate screen and
 * private modes survive a split read, and a buffer over the cap drops whole
 * chunks from the front and starts clean.
 */
export function appendReplayChunk(session: ReplaySession, line: string): ReplaySession {
  const buffer = session.buffer.slice();
  const last = buffer.length - 1;
  if (last >= 0 && buffer[last].length < TERMINAL_REPLAY_CHUNK_CHARS && !isReplayPrefix(buffer[last])) {
    buffer[last] += line;
  } else {
    buffer.push(line);
  }

  const scanned = session.altTail + line;
  const altScreen = altScreenAfter(session.altScreen, scanned);
  const altTail = scanned.slice(-ALT_SCREEN_TAIL_CHARS);
  const modeScanned = session.privateModeTail + line;
  const privateModes = privateModesAfter(session.privateModes, modeScanned);
  const privateModeTail = modeScanned.slice(-PRIVATE_MODE_TAIL_CHARS);

  let total = session.chars + line.length;
  if (total > TERMINAL_REPLAY_MAX_CHARS) {
    while (buffer.length > 1 && total > TERMINAL_REPLAY_MAX_CHARS) {
      total -= buffer[0].length;
      buffer.shift();
    }
    const first = buffer[0];
    if (altScreen) {
      if (first !== undefined && first !== TERMINAL_REPLAY_ALT_PREFIX) {
        buffer.unshift(TERMINAL_REPLAY_ALT_PREFIX);
        total += TERMINAL_REPLAY_ALT_PREFIX.length;
      }
    } else if (first !== undefined && first !== TERMINAL_REPLAY_TRUNCATED_MARKER) {
      const nl = first.indexOf("\n");
      if (nl >= 0) {
        total -= nl + 1;
        buffer[0] = first.slice(nl + 1);
      }
      buffer.unshift(TERMINAL_REPLAY_TRUNCATED_MARKER);
      total += TERMINAL_REPLAY_TRUNCATED_MARKER.length;
    }
  }

  return {
    buffer,
    chars: total,
    altScreen,
    altTail,
    privateModes,
    privateModeTail,
  };
}
