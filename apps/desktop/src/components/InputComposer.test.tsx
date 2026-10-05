import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { InputComposer, SLOW_DRAFT_TEXT } from "./InputComposer";

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

    const input = screen.getByPlaceholderText(/type a command/i);
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

    const input = screen.getByPlaceholderText(/type a command/i);
    await userEvent.type(input, "{Enter}");

    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Shift+Enter does not submit", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer mode="command" onModeChange={() => {}} onSubmit={onSubmit} />,
    );
    const input = screen.getByPlaceholderText(/type a command/i);
    await userEvent.type(input, "git status{Shift>}{Enter}{/Shift}");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("whitespace-only input does not submit, and submitted text is trimmed", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer mode="command" onModeChange={() => {}} onSubmit={onSubmit} />,
    );
    const input = screen.getByPlaceholderText(/type a command/i);
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
    const input = screen.getByPlaceholderText(/type a command/i);
    await userEvent.type(input, "ls{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByText("Working…")).toBeDisabled();
  });

  it("says Ask is drafting, then why it can take a while", () => {
    vi.useFakeTimers();
    try {
      const { rerender } = render(
        <InputComposer mode="ask" onModeChange={() => {}} onSubmit={() => {}} busy />,
      );
      expect(screen.getByPlaceholderText("Drafting a command…")).toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("");
      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(screen.getByPlaceholderText(SLOW_DRAFT_TEXT)).toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent(SLOW_DRAFT_TEXT);
      rerender(<InputComposer mode="ask" onModeChange={() => {}} onSubmit={() => {}} />);
      expect(screen.getByPlaceholderText("Describe what you want to do…")).toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("");
    } finally {
      vi.useRealTimers();
    }
  });

  it.each([
    ["disabled", { disabled: true }],
    ["running", { isRunning: true }],
  ])("does not submit typed text on Enter when %s", async (_name, flags) => {
    const onSubmit = vi.fn();
    const props = { mode: "command" as const, onModeChange: () => {}, onSubmit };
    const { rerender } = render(<InputComposer {...props} />);
    await userEvent.type(screen.getByPlaceholderText(/type a command/i), "ls");

    rerender(<InputComposer {...props} {...flags} />);
    const input = screen.getByRole("textbox");
    expect((input as HTMLTextAreaElement).value).toBe("ls");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("reads Draft plan in Ask, and stays focusable while a command runs", () => {
    const { rerender } = render(
      <InputComposer mode="ask" onModeChange={() => {}} onSubmit={() => {}} />,
    );
    expect(screen.getByRole("group", { name: "How to send this" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ask" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Draft plan" })).toBeInTheDocument();

    rerender(
      <InputComposer mode="command" onModeChange={() => {}} onSubmit={() => {}} isRunning />,
    );
    const box = screen.getByRole("textbox", { name: "Command" });
    expect(box).not.toBeDisabled();
    expect(box).toHaveAttribute("readonly");
    expect(screen.getByRole("button", { name: "Stop the command" })).toBeInTheDocument();
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
