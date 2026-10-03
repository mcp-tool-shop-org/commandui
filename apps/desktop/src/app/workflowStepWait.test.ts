import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { waitForTerminalStatus } from "./workflowStepWait";

type Item = { status: "planned" | "running" | "success" };
const isTerminal = (i: Item) => i.status === "success";

describe("waitForTerminalStatus", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("resolves ok only once the item is terminal, not on a planned item", async () => {
    let current: Item | undefined = { status: "planned" };
    const readItem = vi.fn(() => current);
    const p = waitForTerminalStatus(readItem, isTerminal, new AbortController().signal, 10_000, 100);
    let result: unknown;
    void p.then((r) => (result = r));

    await vi.advanceTimersByTimeAsync(350);
    expect(readItem).toHaveBeenCalled();
    expect(result).toBeUndefined();

    current = { status: "running" };
    await vi.advanceTimersByTimeAsync(200);
    expect(result).toBeUndefined();

    current = { status: "success" };
    await vi.advanceTimersByTimeAsync(100);
    expect(result).toEqual({ ok: true, item: { status: "success" } });
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps polling while the item is missing", async () => {
    let current: Item | undefined;
    const p = waitForTerminalStatus(() => current, isTerminal, new AbortController().signal, 10_000, 100);
    await vi.advanceTimersByTimeAsync(300);
    current = { status: "success" };
    await vi.advanceTimersByTimeAsync(100);
    await expect(p).resolves.toEqual({ ok: true, item: { status: "success" } });
  });

  it("times out for a never-terminal item and cleans up timers", async () => {
    const readItem = vi.fn((): Item => ({ status: "running" }));
    const p = waitForTerminalStatus(readItem, isTerminal, new AbortController().signal, 1_000, 100);
    await vi.advanceTimersByTimeAsync(1_000);
    await expect(p).resolves.toEqual({ ok: false, reason: "timeout" });
    expect(vi.getTimerCount()).toBe(0);

    const calls = readItem.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(readItem.mock.calls.length).toBe(calls);
  });

  it("times out even when readItem never returns an item", async () => {
    const p = waitForTerminalStatus<Item>(() => undefined, isTerminal, new AbortController().signal, 500, 100);
    await vi.advanceTimersByTimeAsync(500);
    await expect(p).resolves.toEqual({ ok: false, reason: "timeout" });
    expect(vi.getTimerCount()).toBe(0);
  });

  it("resolves aborted on abort mid-wait and cleans up", async () => {
    const ac = new AbortController();
    const readItem = vi.fn((): Item => ({ status: "running" }));
    const p = waitForTerminalStatus(readItem, isTerminal, ac.signal, 10_000, 100);
    await vi.advanceTimersByTimeAsync(250);
    ac.abort();
    await expect(p).resolves.toEqual({ ok: false, reason: "aborted" });
    expect(vi.getTimerCount()).toBe(0);

    const calls = readItem.mock.calls.length;
    await vi.advanceTimersByTimeAsync(20_000);
    expect(readItem.mock.calls.length).toBe(calls);
  });

  it("resolves aborted immediately for an already-aborted signal without polling", async () => {
    const ac = new AbortController();
    ac.abort();
    const readItem = vi.fn((): Item => ({ status: "success" }));
    const p = waitForTerminalStatus(readItem, isTerminal, ac.signal, 10_000, 100);
    await expect(p).resolves.toEqual({ ok: false, reason: "aborted" });
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(readItem).not.toHaveBeenCalled();
  });

  it("a late timeout does not overwrite an ok result", async () => {
    const readItem = vi.fn((): Item => ({ status: "success" }));
    const p = waitForTerminalStatus(readItem, isTerminal, new AbortController().signal, 1_000, 100);
    await vi.advanceTimersByTimeAsync(100);
    await expect(p).resolves.toEqual({ ok: true, item: { status: "success" } });
    await vi.advanceTimersByTimeAsync(5_000);
    expect(vi.getTimerCount()).toBe(0);
    expect(readItem).toHaveBeenCalledTimes(1);
  });

  it("removes its abort listener on settle", async () => {
    const ac = new AbortController();
    const remove = vi.spyOn(ac.signal, "removeEventListener");
    const p = waitForTerminalStatus((): Item => ({ status: "success" }), isTerminal, ac.signal, 1_000, 100);
    await vi.advanceTimersByTimeAsync(100);
    await p;
    expect(remove).toHaveBeenCalledWith("abort", expect.any(Function));
  });
});
