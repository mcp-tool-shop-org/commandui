/**
 * Whether the welcome screen opens when CommandUI starts. Stored per user in
 * the app's web storage. Storage can be unavailable (private mode, a locked
 * profile): then the welcome shows, and turning it off lasts for the run.
 */

const KEY = "commandui.welcome.showAtStartup";

export function readShowWelcome(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): boolean {
  try {
    return storage?.getItem(KEY) !== "false";
  } catch {
    return true;
  }
}

export function writeShowWelcome(show: boolean, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(KEY, show ? "true" : "false");
  } catch {
    // Not persisted; the choice still applies until the app closes.
  }
}

function safeStorage(): Storage | undefined {
  try {
    return typeof window !== "undefined" ? window.localStorage : undefined;
  } catch {
    return undefined;
  }
}
