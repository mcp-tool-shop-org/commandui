import { describe, expect, it } from "vitest";
import {
  ALT_SCREEN_TAIL_CHARS,
  PRIVATE_MODE_TAIL_CHARS,
  TERMINAL_REPLAY_ALT_PREFIX,
  TERMINAL_REPLAY_CHUNK_CHARS,
  TERMINAL_REPLAY_MAX_CHARS,
  TERMINAL_REPLAY_TRUNCATED_MARKER,
  altScreenAfter,
  appendReplayChunk,
  emptyReplaySession,
  isReplayPrefix,
  privateModePrefix,
  privateModesAfter,
} from "./terminalReplay";

describe("terminal replay", () => {
  it("coalesces small reads and starts a new chunk at the size cap", () => {
    let session = emptyReplaySession();
    session = appendReplayChunk(session, "ab");
    session = appendReplayChunk(session, "cd");
    expect(session.buffer).toEqual(["abcd"]);
    expect(session.chars).toBe(4);

    const full = "x".repeat(TERMINAL_REPLAY_CHUNK_CHARS);
    session = appendReplayChunk(session, full);
    expect(session.buffer).toEqual(["abcd" + full]);
    session = appendReplayChunk(session, "next");
    expect(session.buffer).toEqual(["abcd" + full, "next"]);
  });

  it("does not append onto a truncation or alternate-screen prefix", () => {
    const truncated = appendReplayChunk(
      { ...emptyReplaySession(), buffer: [TERMINAL_REPLAY_TRUNCATED_MARKER], chars: TERMINAL_REPLAY_TRUNCATED_MARKER.length },
      "kept",
    );
    expect(truncated.buffer).toEqual([TERMINAL_REPLAY_TRUNCATED_MARKER, "kept"]);

    const alt = appendReplayChunk(
      { ...emptyReplaySession(), buffer: [TERMINAL_REPLAY_ALT_PREFIX], chars: TERMINAL_REPLAY_ALT_PREFIX.length, altScreen: true },
      "kept",
    );
    expect(alt.buffer[0]).toBe(TERMINAL_REPLAY_ALT_PREFIX);
    expect(alt.buffer[1]).toBe("kept");
  });

  it("sees an alternate-screen sequence split across two reads", () => {
    const open = "\x1b[?1049h";
    const head = open.slice(0, open.length - 1);
    let session = appendReplayChunk(emptyReplaySession(), `before${head}`);
    expect(session.altScreen).toBe(false);
    expect(session.altTail).toBe((`before${head}`).slice(-ALT_SCREEN_TAIL_CHARS));
    session = appendReplayChunk(session, "hrest");
    expect(session.altScreen).toBe(true);
    session = appendReplayChunk(session, "\x1b[?1049l");
    expect(session.altScreen).toBe(false);
  });

  it("sees a private mode split across two reads and ignores modes the shell does not restore", () => {
    const seq = "\x1b[?2004h";
    let session = appendReplayChunk(emptyReplaySession(), seq.slice(0, -1));
    expect(session.privateModes[2004]).toBeUndefined();
    session = appendReplayChunk(session, "h");
    expect(session.privateModes[2004]).toBe(true);
    expect(session.privateModeTail.length).toBeLessThanOrEqual(PRIVATE_MODE_TAIL_CHARS);

    session = appendReplayChunk(session, "\x1b[?25l\x1b[?2004l");
    expect(session.privateModes[25]).toBeUndefined();
    expect(session.privateModes[2004]).toBe(false);
  });

  it("drops whole chunks from the front and starts the main screen on a clean line", () => {
    const chunk = "line-one\nline-two\n";
    const piece = chunk.repeat(Math.ceil(TERMINAL_REPLAY_CHUNK_CHARS / chunk.length));
    // Truncation clamps the buffer back under the cap, so a while on chars never ends.
    const count = Math.floor(TERMINAL_REPLAY_MAX_CHARS / piece.length) + 2;
    let session = emptyReplaySession();
    for (let i = 0; i < count; i++) session = appendReplayChunk(session, piece);
    expect(session.buffer[0]).toBe(TERMINAL_REPLAY_TRUNCATED_MARKER);
    expect(session.buffer[1]?.startsWith("line-two")).toBe(true);
    expect(session.chars).toBeLessThanOrEqual(
      TERMINAL_REPLAY_MAX_CHARS + TERMINAL_REPLAY_TRUNCATED_MARKER.length,
    );
  });

  it("re-enters the alternate screen when a full-screen session is truncated", () => {
    let session = appendReplayChunk(emptyReplaySession(), "\x1b[?1049h");
    const blob = "y".repeat(TERMINAL_REPLAY_CHUNK_CHARS);
    const count = Math.floor(TERMINAL_REPLAY_MAX_CHARS / blob.length) + 3;
    for (let i = 0; i < count; i++) session = appendReplayChunk(session, blob);
    expect(session.altScreen).toBe(true);
    expect(session.buffer[0]).toBe(TERMINAL_REPLAY_ALT_PREFIX);
    expect(session.buffer.includes(TERMINAL_REPLAY_TRUNCATED_MARKER)).toBe(false);
  });

  it("builds the prefix that puts a reset terminal back into the shell's modes", () => {
    expect(privateModePrefix(undefined)).toBe("");
    expect(privateModePrefix({})).toBe("");
    const prefix = privateModePrefix({ 2004: true, 1: false });
    expect(prefix).toContain("\x1b[?2004h");
    expect(prefix).toContain("\x1b[?1l");
  });

  it("tracks only the last alternate-screen and private-mode value", () => {
    expect(altScreenAfter(false, "\x1b[?47h\x1b[?1047l")).toBe(false);
    expect(altScreenAfter(true, "plain")).toBe(true);
    const modes = privateModesAfter({}, "\x1b[?1000;1002h\x1b[?1000l");
    expect(modes[1000]).toBe(false);
    expect(modes[1002]).toBe(true);
    expect(isReplayPrefix(TERMINAL_REPLAY_TRUNCATED_MARKER)).toBe(true);
    expect(isReplayPrefix("output")).toBe(false);
  });

  it("does not mutate the caller's buffer", () => {
    const start = emptyReplaySession();
    appendReplayChunk(start, "hello");
    expect(start.buffer).toEqual([]);
    expect(start.chars).toBe(0);
  });
});
