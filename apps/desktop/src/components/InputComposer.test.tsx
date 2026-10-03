import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { InputComposer } from "./InputComposer";

describe("InputComposer", () => {
  it("submits on Enter", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={() => {}}
        onSubmit={onSubmit}
      />,
    );

    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "git status{Enter}");

    expect(onSubmit).toHaveBeenCalledWith("git status");
  });

  it("switches mode on button click", async () => {
    const onModeChange = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={onModeChange}
        onSubmit={() => {}}
      />,
    );

    await userEvent.click(screen.getByText("Ask"));
    expect(onModeChange).toHaveBeenCalledWith("ask");
  });

  it("does not submit empty input", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={() => {}}
        onSubmit={onSubmit}
      />,
    );

    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "{Enter}");

    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Shift+Enter does not submit", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer mode="command" onModeChange={() => {}} onSubmit={onSubmit} />,
    );
    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "git status{Shift>}{Enter}{/Shift}");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("whitespace-only input does not submit, and submitted text is trimmed", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer mode="command" onModeChange={() => {}} onSubmit={onSubmit} />,
    );
    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "   {Enter}");
    expect(onSubmit).not.toHaveBeenCalled();

    await userEvent.type(input, "  ls  {Enter}");
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith("ls");
  });

  it("does not submit on Enter when busy", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={() => {}}
        onSubmit={onSubmit}
        busy
      />,
    );
    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "ls{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByText("Working…")).toBeDisabled();
  });

  it.each([
    ["disabled", { disabled: true }],
    ["running", { isRunning: true }],
  ])("does not submit typed text on Enter when %s", async (_name, flags) => {
    const onSubmit = vi.fn();
    const props = { mode: "command" as const, onModeChange: () => {}, onSubmit };
    const { rerender } = render(<InputComposer {...props} />);
    await userEvent.type(screen.getByPlaceholderText(/submit a command/i), "ls");

    rerender(<InputComposer {...props} {...flags} />);
    const input = screen.getByRole("textbox");
    expect((input as HTMLTextAreaElement).value).toBe("ls");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("shows Stop instead of Run while running and Stop calls onInterrupt", async () => {
    const onInterrupt = vi.fn();
    const onSubmit = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={() => {}}
        onSubmit={onSubmit}
        isRunning
        onInterrupt={onInterrupt}
      />,
    );
    expect(screen.queryByText("Run")).toBeNull();
    await userEvent.click(screen.getByText("Stop"));
    expect(onInterrupt).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
