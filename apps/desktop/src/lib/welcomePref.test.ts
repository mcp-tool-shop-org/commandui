import { afterEach, describe, expect, it } from "vitest";
import { readShowWelcome, writeShowWelcome } from "./welcomePref";

function memoryStorage() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  };
}

const throwing = {
  getItem: () => {
    throw new Error("storage locked");
  },
  setItem: () => {
    throw new Error("storage locked");
  },
};

afterEach(() => window.localStorage.clear());

describe("welcome preference", () => {
  it("shows the welcome until the user turns it off", () => {
    const store = memoryStorage();
    expect(readShowWelcome(store)).toBe(true);
    writeShowWelcome(false, store);
    expect(readShowWelcome(store)).toBe(false);
    writeShowWelcome(true, store);
    expect(readShowWelcome(store)).toBe(true);
  });

  it("uses the app's local storage by default", () => {
    expect(readShowWelcome()).toBe(true);
    writeShowWelcome(false);
    expect(window.localStorage.getItem("commandui.welcome.showAtStartup")).toBe("false");
    expect(readShowWelcome()).toBe(false);
  });

  it("shows the welcome, and does not throw, when storage fails", () => {
    expect(readShowWelcome(throwing)).toBe(true);
    expect(() => writeShowWelcome(false, throwing)).not.toThrow();
  });

  it("treats missing storage as show", () => {
    expect(readShowWelcome(undefined)).toBe(true);
    expect(() => writeShowWelcome(false, undefined)).not.toThrow();
  });
});
