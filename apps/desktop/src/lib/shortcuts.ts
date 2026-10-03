/**
 * Centralized shortcut registry — pure logic, zero React deps.
 * Parses combo strings, matches KeyboardEvents, resolves by context priority.
 */

import type { FocusZone } from "@commandui/state";

export type ShortcutContext = "global" | FocusZone;

export type ShortcutDef = {
  id: string;
  combo: string;
  context: ShortcutContext[];
  when?: () => boolean;
  action: () => void;
};

export type ParsedCombo = {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
  key: string; // normalized lowercase
};

const MODIFIER_KEYS = new Set(["ctrl", "shift", "alt", "meta", "cmd"]);

/** Parse a combo string like "ctrl+k" or "shift+enter" into structured form. */
export function parseCombo(combo: string): ParsedCombo {
  const parts = combo.toLowerCase().split("+").map((p) => p.trim());
  const parsed: ParsedCombo = {
    ctrl: false,
    shift: false,
    alt: false,
    meta: false,
    key: "",
  };

  for (const part of parts) {
    if (part === "ctrl" || part === "cmd") {
      // On Windows, cmd maps to ctrl
      parsed.ctrl = true;
    } else if (part === "shift") {
      parsed.shift = true;
    } else if (part === "alt") {
      parsed.alt = true;
    } else if (part === "meta") {
      parsed.meta = true;
    } else {
      parsed.key = part;
    }
  }

  return parsed;
}

/** Check whether a parsed combo has any modifier keys. */
export function hasModifier(parsed: ParsedCombo): boolean {
  return parsed.ctrl || parsed.shift || parsed.alt || parsed.meta;
}

/** Normalize a KeyboardEvent key to match our combo keys. */
function normalizeKey(event: KeyboardEvent): string {
  const key = event.key.toLowerCase();
  // Map common aliases
  if (key === " ") return "space";
  if (key === ",") return ",";
  if (key === ".") return ".";
  return key;
}

/** Test if a parsed combo matches a KeyboardEvent. */
export function matchesEvent(
  parsed: ParsedCombo,
  event: KeyboardEvent,
): boolean {
  if (parsed.ctrl !== (event.ctrlKey || event.metaKey)) return false;
  if (parsed.shift !== event.shiftKey) return false;
  if (parsed.alt !== event.altKey) return false;
  return normalizeKey(event) === parsed.key;
}

/** Whether a combo is a "bare key" (no modifiers, single character or special key). */
function isBareKey(parsed: ParsedCombo): boolean {
  return !hasModifier(parsed);
}

/** Plain Ctrl+<letter|digit|comma> (no Shift/Alt): a control sequence the shell reads. */
export function isShellChord(event: KeyboardEvent): boolean {
  return (
    event.ctrlKey &&
    !event.shiftKey &&
    !event.altKey &&
    !event.metaKey &&
    /^[a-z0-9,]$/i.test(event.key)
  );
}

/** Special keys that should work even in text input zones. */
const SPECIAL_KEYS = new Set(["escape", "enter", "tab"]);

const NON_TEXT_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "radio",
  "submit",
  "reset",
  "file",
  "color",
  "range",
  "image",
  "hidden",
]);

/** True for editors where a bare letter should type, not trigger a shortcut. */
function isTextFieldElement(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  if (tag === "TEXTAREA" || tag === "SELECT") return true;
  if (tag !== "INPUT") return false;
  const type = (target as HTMLInputElement).type.toLowerCase();
  return !NON_TEXT_INPUT_TYPES.has(type);
}

/**
 * Focus can sit in the plan editor, palette, history search, or workflow
 * editor while the focus zone is still "plan". Bare keys must not fire there.
 */
function eventInTextField(event: KeyboardEvent): boolean {
  if (isTextFieldElement(event.target)) return true;
  if (typeof document === "undefined") return false;
  const active = document.activeElement;
  return active !== event.target && isTextFieldElement(active);
}

/**
 * Resolve the best-matching shortcut for a keyboard event given the current focus zone.
 *
 * Rules:
 * - Zone-specific context beats "global"
 * - When zone is "terminal" or "composer", bare-key shortcuts are suppressed
 *   (except special keys like Escape/Enter)
 * - Bare keys are also suppressed while focus is in any text field
 * - `when` guard must return true (or be absent)
 */
export function resolveShortcut(
  defs: ShortcutDef[],
  event: KeyboardEvent,
  currentZone: FocusZone | null,
): ShortcutDef | null {
  let globalMatch: ShortcutDef | null = null;

  for (const def of defs) {
    const parsed = parseCombo(def.combo);
    if (!matchesEvent(parsed, event)) continue;
    if (def.when && !def.when()) continue;

    // Guard: suppress bare-key shortcuts in text-input zones and text fields
    const isTextZone = currentZone === "terminal" || currentZone === "composer";
    if (
      (isTextZone || eventInTextField(event)) &&
      isBareKey(parsed) &&
      !SPECIAL_KEYS.has(parsed.key)
    ) {
      continue;
    }

    // Zone-specific match takes priority
    if (currentZone && def.context.includes(currentZone)) {
      return def;
    }

    // Plain Ctrl+<letter> belongs to the shell while the terminal has focus
    // (^W word-rubout, ^K kill-line, ^M enter, ...). A global app shortcut must not
    // also fire; app actions use Ctrl+Shift+<letter> there. Zone-specific defs still match.
    if (currentZone === "terminal" && isShellChord(event)) continue;

    // Track first global match as fallback
    if (!globalMatch && def.context.includes("global")) {
      globalMatch = def;
    }
  }

  return globalMatch;
}
