import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ErrorBoundary } from "./ErrorBoundary";

function Boom({ message }: { message: string }): never {
  throw new Error(message);
}

describe("ErrorBoundary", () => {
  it("shows the failure, copies the details, and reloads", async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
    const navigationErrors: string[] = [];
    const virtualConsole = (window as unknown as { _virtualConsole?: { on: (event: string, fn: (error: Error) => void) => void } })._virtualConsole;
    virtualConsole?.on("jsdomError", (error) => navigationErrors.push(error.message));
    const error = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <ErrorBoundary>
        <Boom message="render blew up" />
      </ErrorBoundary>,
    );

    expect(screen.getByRole("heading", { name: "CommandUI encountered an unexpected error" })).toBeInTheDocument();
    expect(screen.getByText("render blew up")).toBeInTheDocument();
    expect(error).toHaveBeenCalledWith(
      "[CommandUI] Uncaught render error:",
      expect.objectContaining({ type: "render_error", message: "render blew up" }),
    );

    expect(screen.queryByText(/Boom/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show details" }));
    expect(document.querySelector("pre.error-boundary-message")?.textContent).toMatch(/render blew up/);
    await user.click(screen.getByRole("button", { name: "Hide details" }));
    expect(document.querySelector("pre.error-boundary-message")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Copy Error Details" }));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("render blew up"));

    await user.click(screen.getByRole("button", { name: "Reload App" }));
    expect(navigationErrors).toContain("Not implemented: navigation (except hash changes)");

    error.mockRestore();
  });

  it("renders its children when nothing throws", () => {
    render(
      <ErrorBoundary>
        <p>still here</p>
      </ErrorBoundary>,
    );
    expect(screen.getByText("still here")).toBeInTheDocument();
  });
});
