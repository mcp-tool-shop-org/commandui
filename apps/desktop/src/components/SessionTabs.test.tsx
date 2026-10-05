import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { SessionSummary } from "@commandui/domain";
import { SessionTabs } from "./SessionTabs";

const sessions = [
  { id: "s1", label: "Session 1", cwd: "/", shell: "pwsh", status: "active", createdAt: "", lastActiveAt: "" },
  { id: "s2", label: "Session 2", cwd: "/", shell: "pwsh", status: "active", createdAt: "", lastActiveAt: "" },
] as SessionSummary[];

describe("SessionTabs", () => {
  it("selects, creates, and closes a session, and marks one that has exited", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    const onCreate = vi.fn();
    const onClose = vi.fn();
    render(
      <SessionTabs
        sessions={sessions}
        activeSessionId="s1"
        onSelect={onSelect}
        onCreate={onCreate}
        onClose={onClose}
        exitedSessionIds={new Set(["s2"])}
      />,
    );

    expect(screen.getByRole("tab", { name: "Session 2 (exited)" })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Session 2 (exited)" }));
    expect(onSelect).toHaveBeenCalledWith("s2");

    await user.click(screen.getByRole("button", { name: "Close Session 1" }));
    expect(onClose).toHaveBeenCalledWith("s1");

    await user.click(screen.getByRole("button", { name: "New session" }));
    expect(onCreate).toHaveBeenCalledOnce();
  });
});
